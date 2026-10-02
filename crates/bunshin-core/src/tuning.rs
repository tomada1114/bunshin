//! The app's tunables, shared by every feature.
use jiff::civil::Time;

const DEFAULT_DAY_BOUNDARY: Time = Time::constant(4, 0, 0, 0);

/// Tunables in one place (designing-core-logic). Built only by [`Tuning::new`] (or
/// [`Default`]), so every `Tuning` holds a range with at least one value in it.
///
/// ```
/// use bunshin_core::{Counter, Tuning, TuningError};
///
/// let tuning = Tuning::new(0, 3)?;
/// assert_eq!(Counter::new(7, tuning).value(), 3);
/// assert_eq!(Tuning::new(5, 1), Err(TuningError::MinAboveMax));
/// # Ok::<(), TuningError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tuning {
    counter: CounterTuning,
    /// Task limits and session undo capacity.
    pub day: DayTuning,
    /// 04:00 keeps late-night work on the preceding logical day; callers may tune it.
    pub day_boundary: Time,
}

impl Tuning {
    /// A range from `min` to `max`, both included.
    ///
    /// # Errors
    /// [`TuningError::MinAboveMax`] when `min > max`: no value lies in that range, so a
    /// counter could not stay inside it.
    pub const fn new(min: i64, max: i64) -> Result<Self, TuningError> {
        if min > max {
            return Err(TuningError::MinAboveMax);
        }
        Ok(Self {
            counter: CounterTuning { min, max },
            day: DayTuning::shipped(),
            day_boundary: DEFAULT_DAY_BOUNDARY,
        })
    }

    /// The lowest value; also the value a fresh or reset counter holds.
    #[must_use]
    pub const fn min(&self) -> i64 {
        self.counter.min
    }

    /// The highest value.
    #[must_use]
    pub const fn max(&self) -> i64 {
        self.counter.max
    }
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            counter: CounterTuning { min: 0, max: 99 },
            day: DayTuning::shipped(),
            day_boundary: DEFAULT_DAY_BOUNDARY,
        }
    }
}

/// A [`Tuning`] that no counter could stay inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TuningError {
    /// `min` is above `max`, so the range holds no value.
    #[error("the tuning's minimum is above its maximum")]
    MinAboveMax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CounterTuning {
    min: i64,
    max: i64,
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
