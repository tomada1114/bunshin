//! Pure key and request state for the in-memory board screen.

use super::{InputBuffer, KeyRegion, ScreenAction, ScreenKey, action_for, viewport::BoardViewport};
use crate::{
    ModelAnswer, ModelError, ModelRequest, Now, Tuning, UnixMillis,
    board::{Board, FailureKind, Outcome, Rng, Turn},
};

/// A request the binary sends to its single model worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRequest {
    /// Completion token returned by the worker.
    pub id: u64,
    /// The board turn's schema-backed model request.
    pub request: ModelRequest,
}

/// Where the board screen currently sends navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardFocus {
    /// The input line accepts text.
    Input,
    /// The board accepts scrolling and the quit key.
    Board,
}

/// The model failure retained for the header until a turn succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardFailure {
    /// A typed error returned by the model port.
    Model(ModelError),
    /// A successful response contained an empty body.
    Kind(FailureKind),
}

/// Compact status drawn at the right side of the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardStatus {
    /// No recent failure and no long-running turn.
    Idle,
    /// The active model request has exceeded the thinking delay.
    Writing,
    /// The most recent failure, retained until a successful post.
    Failure(BoardFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Flight {
    id: u64,
    turn: Turn,
    started: UnixMillis,
}

/// The board, text input, turn scheduler, and viewport for the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardScreen {
    board: Board,
    rng: Rng,
    tuning: Tuning,
    input: InputBuffer,
    focus: BoardFocus,
    finished: bool,
    next_request_id: u64,
    flight: Option<Flight>,
    last_failure: Option<BoardFailure>,
    pub(super) viewport: BoardViewport,
}

impl BoardScreen {
    /// Create a fresh, empty board and seed its deterministic turn generator.
    #[must_use]
    pub fn new(tuning: Tuning, seed: u64) -> Self {
        Self {
            board: Board::new(tuning.board),
            rng: Rng::from_seed(seed),
            tuning,
            input: InputBuffer::default(),
            focus: BoardFocus::Input,
            finished: false,
            next_request_id: 1,
            flight: None,
            last_failure: None,
            viewport: BoardViewport::default(),
        }
    }

    /// Board posts in chronological order.
    #[must_use]
    pub const fn board(&self) -> &Board {
        &self.board
    }

    /// Current literal input.
    #[must_use]
    pub const fn input(&self) -> &InputBuffer {
        &self.input
    }

    /// The region receiving navigation keys.
    #[must_use]
    pub const fn focus(&self) -> BoardFocus {
        self.focus
    }

    /// Whether an immediate quit key was accepted.
    #[must_use]
    pub const fn finished(&self) -> bool {
        self.finished
    }

    /// Maximum input length in Unicode scalar values.
    #[must_use]
    pub const fn input_limit(&self) -> usize {
        self.tuning.board.input_max_chars
    }

    /// Current header status, using the caller's clock sample.
    #[must_use]
    pub fn status(&self, at: UnixMillis) -> BoardStatus {
        if let Some(flight) = &self.flight {
            let elapsed = i128::from(at.0) - i128::from(flight.started.0);
            let thinking_after =
                i128::try_from(self.tuning.chat.thinking_after.as_millis()).unwrap_or(i128::MAX);
            if elapsed >= thinking_after {
                return BoardStatus::Writing;
            }
        }
        self.last_failure
            .map_or(BoardStatus::Idle, BoardStatus::Failure)
    }

    /// Record wrapped rows measured by the terminal renderer.
    pub fn record_board_layout(&mut self, rows: usize, height: usize) {
        self.viewport.record_layout(rows, height);
    }

    /// Top wrapped row when the board is scrolled away from its newest post.
    #[must_use]
    pub const fn board_scroll_top(&self) -> usize {
        self.viewport.top()
    }

    /// Whether the board follows newly appended posts.
    #[must_use]
    pub const fn board_follows_latest(&self) -> bool {
        self.viewport.follows_latest()
    }

    /// Number of posts added since scrolling away from the latest post.
    #[must_use]
    pub fn board_new_posts(&self) -> usize {
        self.viewport.new_post_count(self.board.posts())
    }

    /// Index of the first post added while the viewport was paused.
    #[must_use]
    pub fn board_first_unseen_post(&self) -> Option<usize> {
        self.viewport.first_unseen_post(self.board.posts())
    }

    /// Start a due turn if no model request is in flight.
    pub fn prepare_turn(&mut self, now: Now) -> Option<BoardRequest> {
        if self.finished {
            return None;
        }
        let turn = self.board.next_turn(now.instant, &mut self.rng)?;
        let Some(request) = self.board.request(&turn, self.tuning) else {
            let outcome = self
                .board
                .finish(&turn, Err(ModelError::Failed), now.instant);
            if let Outcome::Failed(kind) = outcome {
                self.last_failure = Some(BoardFailure::Kind(kind));
            }
            return None;
        };
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        self.flight = Some(Flight {
            id,
            turn,
            started: now.instant,
        });
        Some(BoardRequest { id, request })
    }

    /// Apply one worker completion to its reserved board turn.
    pub fn finish_turn(
        &mut self,
        id: u64,
        answer: Result<ModelAnswer, ModelError>,
        now: Now,
    ) -> Outcome {
        if self.flight.as_ref().is_none_or(|flight| flight.id != id) {
            return Outcome::Stale;
        }
        let Some(flight) = self.flight.take() else {
            return Outcome::Stale;
        };
        let model_error = answer.as_ref().err().copied();
        let outcome = self.board.finish(&flight.turn, answer, now.instant);
        match &outcome {
            Outcome::Posted(_) => self.last_failure = None,
            Outcome::Failed(kind) => {
                self.last_failure = Some(
                    model_error.map_or_else(|| BoardFailure::Kind(*kind), BoardFailure::Model),
                );
            }
            Outcome::Stale => {}
        }
        outcome
    }

    /// Apply a key without touching the terminal or model.
    pub fn update(&mut self, key: ScreenKey, now: Now) {
        let region = match self.focus {
            BoardFocus::Input => KeyRegion::Input,
            BoardFocus::Board => KeyRegion::Board,
        };
        let action = action_for(key, region);
        match action {
            Some(ScreenAction::Quit) => self.finished = true,
            Some(ScreenAction::ToggleFocus) => {
                self.focus = match self.focus {
                    BoardFocus::Input => BoardFocus::Board,
                    BoardFocus::Board => BoardFocus::Input,
                };
            }
            Some(ScreenAction::ScrollOlder) => match key.normalized() {
                ScreenKey::Up => self.viewport.scroll_up(self.board.posts()),
                ScreenKey::PageUp => self.viewport.scroll_page_up(self.board.posts()),
                ScreenKey::Char(_)
                | ScreenKey::Down
                | ScreenKey::Left
                | ScreenKey::Right
                | ScreenKey::Enter
                | ScreenKey::Tab
                | ScreenKey::BackTab
                | ScreenKey::Esc
                | ScreenKey::Interrupt
                | ScreenKey::Backspace
                | ScreenKey::Delete
                | ScreenKey::Home
                | ScreenKey::End
                | ScreenKey::PageDown => {}
            },
            Some(ScreenAction::ScrollNewer) => match key.normalized() {
                ScreenKey::Down => self.viewport.scroll_down(self.board.posts()),
                ScreenKey::PageDown => self.viewport.scroll_page_down(self.board.posts()),
                ScreenKey::Char(_)
                | ScreenKey::Up
                | ScreenKey::Left
                | ScreenKey::Right
                | ScreenKey::Enter
                | ScreenKey::Tab
                | ScreenKey::BackTab
                | ScreenKey::Esc
                | ScreenKey::Interrupt
                | ScreenKey::Backspace
                | ScreenKey::Delete
                | ScreenKey::Home
                | ScreenKey::End
                | ScreenKey::PageUp => {}
            },
            Some(ScreenAction::Latest) => self.viewport.follow_latest(),
            Some(ScreenAction::FocusInput) => self.focus = BoardFocus::Input,
            Some(ScreenAction::SubmitInput) => self.submit_input(now.instant),
            Some(ScreenAction::ClearInput) | None if self.focus == BoardFocus::Input => {
                self.input.edit(key, self.input_limit());
            }
            Some(ScreenAction::ClearInput) | None => {}
        }
    }

    fn submit_input(&mut self, at: UnixMillis) {
        let text = self.input.take();
        self.board.owner_post(&text, at);
    }
}
