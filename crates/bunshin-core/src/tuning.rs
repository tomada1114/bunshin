//! The app's tunables, shared by every feature.
use jiff::civil::Time;
use std::time::Duration;

const DEFAULT_MODEL_TIMEOUT: Duration = Duration::from_secs(30);

const DEFAULT_DAY_BOUNDARY: Time = Time::constant(4, 0, 0, 0);

/// Shared domain tunables, adjustable without reading configuration in core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tuning {
    /// Check-in scheduling and guard bounds.
    pub checkin: CheckinTuning,
    /// One hour of quiet from the task-pane mute key, shared with check-in logic.
    pub key_mute_minutes: u16,
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
            checkin: CheckinTuning::shipped(),
            key_mute_minutes: 60,
            day: DayTuning::shipped(),
            day_boundary: DEFAULT_DAY_BOUNDARY,
            model_timeout: DEFAULT_MODEL_TIMEOUT,
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
