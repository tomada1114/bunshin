//! The app's tunables, shared by every feature.
use jiff::civil::Time;

const DEFAULT_DAY_BOUNDARY: Time = Time::constant(4, 0, 0, 0);

/// Shared domain tunables, adjustable without reading configuration in core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tuning {
    /// Task limits and session undo capacity.
    pub day: DayTuning,
    /// 04:00 keeps late-night work on the preceding logical day; callers may tune it.
    pub day_boundary: Time,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            day: DayTuning::shipped(),
            day_boundary: DEFAULT_DAY_BOUNDARY,
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
