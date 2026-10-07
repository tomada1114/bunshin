//! The app's tunables, shared by every feature.
use jiff::civil::Time;
use std::time::Duration;

const DEFAULT_MODEL_TIMEOUT: Duration = Duration::from_secs(30);

const DEFAULT_DAY_BOUNDARY: Time = Time::constant(4, 0, 0, 0);

/// Shared domain tunables, adjustable without reading configuration in core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tuning {
    /// Prompt window, response reserves and text bounds shared by model callers.
    pub prompt: PromptTuning,
    /// Owner-call feedback and unavailable-model recheck intervals.
    pub chat: ChatTuning,
    /// Task limits and session undo capacity.
    pub day: DayTuning,
    /// 04:00 keeps late-night work on the preceding logical day; callers may tune it.
    pub day_boundary: Time,
    /// Maximum model call and availability-probe wait; callers may tune it.
    pub model_timeout: Duration,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            prompt: PromptTuning::default(),
            chat: ChatTuning::default(),
            day: DayTuning::shipped(),
            day_boundary: DEFAULT_DAY_BOUNDARY,
            model_timeout: DEFAULT_MODEL_TIMEOUT,
        }
    }
}

/// Owner-call feedback delays; time is supplied by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatTuning {
    /// Terminal polling bound for worker completions and feedback deadlines.
    pub poll_interval: Duration,
    /// Fast answers never show a thinking row.
    pub thinking_after: Duration,
    /// Replace the thinking row with a cancellation hint.
    pub long_wait_after: Duration,
    /// Interval between probes while the model cannot be used.
    pub availability_recheck: Duration,
}
impl Default for ChatTuning {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            thinking_after: Duration::from_millis(300),
            long_wait_after: Duration::from_secs(10),
            availability_recheck: Duration::from_secs(600),
        }
    }
}

/// Conservative context budgeting, independent of the platform's tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptTuning {
    /// Complete instructions, prompt, schema and answer window.
    pub context_tokens: usize,
    /// Rounded-up ASCII scalars per estimated token.
    pub ascii_chars_per_token: usize,
    /// Reserved answer tokens for an owner's chat call.
    pub chat_answer_tokens: usize,
    /// Maximum owner-message Unicode scalar count, refused before assembly.
    pub input_max_chars: usize,
    /// Reply bound requested in operating rules, never used to truncate an answer.
    pub reply_max_chars: usize,
}
impl Default for PromptTuning {
    fn default() -> Self {
        Self {
            context_tokens: 4096,
            ascii_chars_per_token: 2,
            chat_answer_tokens: 450,
            input_max_chars: 400,
            reply_max_chars: 200,
        }
    }
}

/// Tunable task bounds, independently adjustable for small deterministic tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayTuning {
    /// Eighty Unicode scalar values keep a title compact in the task pane.
    pub title_max_chars: usize,
    /// Fifty total creations bound one day; deletion and undo never restore this budget.
    pub tasks_per_day: usize,
    /// Twenty changes can be undone during one session.
    pub undo_depth: usize,
}
impl DayTuning {
    const fn shipped() -> Self {
        Self {
            title_max_chars: 80,
            tasks_per_day: 50,
            undo_depth: 20,
        }
    }
}
impl Default for DayTuning {
    fn default() -> Self {
        Self::shipped()
    }
}
