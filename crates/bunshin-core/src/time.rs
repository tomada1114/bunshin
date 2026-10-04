//! One clock reading supplies an instant and its local civil date-time.

use jiff::civil::{Date, DateTime, Time};
use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch. Core never reads the clock itself (see [`Clock`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnixMillis(pub i64);

/// Both views of one wall-clock read, so a zone change cannot distort an elapsed gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Now {
    /// Milliseconds since the Unix epoch, used for gaps and timeouts.
    pub instant: UnixMillis,
    /// The same instant in the local zone, used for dates, deadlines, and active hours.
    pub local: DateTime,
}
impl Now {
    /// Convert another instant using this sample's fixed UTC offset. The system
    /// clock overrides this approximation with its zone's historical rules.
    #[must_use]
    pub fn at_fixed_offset(self, instant: UnixMillis) -> Option<Self> {
        let delta = instant.0.checked_sub(self.instant.0)?;
        let local = self
            .local
            .checked_add(jiff::SignedDuration::from_millis(delta))
            .ok()?;
        Some(Self { instant, local })
    }
}

/// The source of the current time: `SystemClock` (platform) in the app, `FixedClock`
/// (test-support) in tests. Injected so a test never sleeps and never depends on the date.
/// It is a wall clock: a later read may return an earlier time when the system time is
/// corrected, so nothing may depend on two reads being in order.
pub trait Clock: Send + Sync {
    /// Both views of the same sampled instant, at millisecond precision. The local
    /// view differs by a valid UTC offset; its subsecond part agrees with the instant.
    /// A zone change can move the local view without moving the instant.
    fn now(&self) -> Now;
    /// Local display time for a stored instant, without advancing the clock.
    /// The default keeps the current sample's offset; zone-aware adapters
    /// override it to account for historical daylight-saving transitions.
    fn local_at(&self, instant: UnixMillis) -> Option<Now> {
        self.now().at_fixed_offset(instant)
    }
}

/// The day an owner is still working on, changing at `boundary` in local civil time.
/// At the earliest representable date, an earlier logical day saturates at [`Date::MIN`].
#[must_use]
pub fn logical_date(local: DateTime, boundary: Time) -> Date {
    let date = local.date();
    if local.time() < boundary {
        date.yesterday().unwrap_or(Date::MIN)
    } else {
        date
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::{Date, date, time};

    use super::logical_date;
    use crate::Tuning;

    #[test]
    fn logical_date_changes_at_four_in_the_morning() {
        let boundary = Tuning::default().day_boundary;
        assert_eq!(boundary, time(4, 0, 0, 0));
        for (local, expected) in [
            (date(2026, 10, 2).at(0, 0, 0, 0), date(2026, 10, 1)),
            (
                date(2026, 10, 2).at(3, 59, 59, 999_999_999),
                date(2026, 10, 1),
            ),
            (date(2026, 10, 2).at(4, 0, 0, 0), date(2026, 10, 2)),
            (date(2026, 10, 2).at(4, 0, 0, 1), date(2026, 10, 2)),
        ] {
            assert_eq!(logical_date(local, boundary), expected, "{local}");
        }
    }

    #[test]
    fn logical_date_crosses_month_year_and_leap_day_boundaries() {
        for (local, expected) in [
            (date(2026, 11, 1).at(3, 59, 59, 0), date(2026, 10, 31)),
            (date(2027, 1, 1).at(0, 0, 0, 0), date(2026, 12, 31)),
            (date(2024, 3, 1).at(1, 0, 0, 0), date(2024, 2, 29)),
            (date(2026, 3, 1).at(1, 0, 0, 0), date(2026, 2, 28)),
        ] {
            assert_eq!(logical_date(local, time(4, 0, 0, 0)), expected, "{local}");
        }
    }

    #[test]
    fn logical_date_uses_the_boundary_passed_by_the_caller() {
        let tuning = Tuning {
            day_boundary: time(6, 0, 0, 0),
            ..Tuning::default()
        };
        assert_eq!(
            logical_date(date(2026, 10, 2).at(5, 59, 59, 0), tuning.day_boundary),
            date(2026, 10, 1)
        );
        assert_eq!(
            logical_date(date(2026, 10, 2).at(6, 0, 0, 0), tuning.day_boundary),
            date(2026, 10, 2)
        );
        assert_eq!(
            logical_date(date(2026, 10, 2).at(0, 0, 0, 0), time(0, 0, 0, 0)),
            date(2026, 10, 2)
        );
    }

    #[test]
    fn logical_date_stays_representable_at_the_minimum_date() {
        assert_eq!(
            logical_date(Date::MIN.at(0, 0, 0, 0), time(4, 0, 0, 0)),
            Date::MIN
        );
    }
}
