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
    /// Check-in scheduling and guard bounds.
    pub checkin: CheckinTuning,
    /// Reaction window for implicit answers, mute reactions and ignored reporting.
    pub inbox_reaction_minutes: u16,
    /// One hour of quiet from the task-pane mute key, shared with check-in logic.
    pub key_mute_minutes: u16,
    /// Task limits and session undo capacity.
    pub day: DayTuning,
    /// 04:00 keeps late-night work on the preceding logical day; callers may tune it.
    pub day_boundary: Time,
    /// Maximum model call and availability-probe wait; callers may tune it.
    pub model_timeout: Duration,
    /// Six hundred Unicode scalar values reserve a compact owner instructions block.
    pub instructions_max_chars: usize,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            prompt: PromptTuning::default(),
            checkin: CheckinTuning::shipped(),
            key_mute_minutes: 60,
            inbox_reaction_minutes: 15,
            day: DayTuning::shipped(),
            day_boundary: DEFAULT_DAY_BOUNDARY,
            model_timeout: DEFAULT_MODEL_TIMEOUT,
            instructions_max_chars: 600,
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
    /// Reserved answer tokens for a later check-in call.
    pub checkin_answer_tokens: usize,
    /// Maximum yesterday summary estimate.
    pub yesterday_tokens: usize,
    /// Number of recent unprompted states included in context.
    pub recent_unprompted: usize,
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
            checkin_answer_tokens: 300,
            yesterday_tokens: 120,
            recent_unprompted: 5,
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

/// Civil scheduling and instant-based check-in intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckinTuning {
    /// Seconds between evaluations.
    pub tick_seconds: u16,
    /// Minutes before a deadline to consider it.
    pub before_deadline_minutes: u16,
    /// Minimum proposed look delay.
    pub planned_min_minutes: u16,
    /// Maximum proposed look delay.
    pub planned_max_minutes: u16,
    /// Delay when the model supplies none.
    pub planned_default_minutes: u16,
    /// Inclusive civil opening time.
    pub active_start: Time,
    /// Exclusive civil closing time.
    pub active_end: Time,
    /// Minimum elapsed minutes between delivered messages.
    pub minimum_gap_minutes: u16,
    /// Elapsed minutes suppressing another message about the same task.
    pub same_task_minutes: u16,
    /// Minimum chat mute duration.
    pub chat_mute_min_minutes: u16,
    /// Maximum chat mute duration.
    pub chat_mute_max_minutes: u16,
    /// A gap strictly larger than this is a sleep catch-up.
    pub sleep_gap_minutes: u16,
}
impl CheckinTuning {
    const fn shipped() -> Self {
        Self {
            tick_seconds: 60,
            before_deadline_minutes: 30,
            planned_min_minutes: 5,
            planned_max_minutes: 120,
            planned_default_minutes: 120,
            active_start: Time::constant(8, 0, 0, 0),
            active_end: Time::constant(22, 0, 0, 0),
            minimum_gap_minutes: 5,
            same_task_minutes: 30,
            chat_mute_min_minutes: 5,
            chat_mute_max_minutes: 480,
            sleep_gap_minutes: 5,
        }
    }
}
impl Default for CheckinTuning {
    fn default() -> Self {
        Self::shipped()
    }
}
