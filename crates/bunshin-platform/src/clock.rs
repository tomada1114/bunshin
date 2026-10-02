use std::time::{SystemTime, UNIX_EPOCH};

use bunshin_core::{Clock, Now, UnixMillis};
use jiff::{SignedDuration, Timestamp, tz::TimeZone};

/// The wall clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Now {
        reading_at(SystemTime::now(), &TimeZone::system())
    }
}

fn reading_at(time: SystemTime, zone: &TimeZone) -> Now {
    // Keep the existing epoch fallback for clocks before 1970, and for samples
    // outside jiff's civil range: a recoverable timestamp must not become a panic.
    let since_epoch = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let millis = i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX);
    let timestamp = Timestamp::from_duration(SignedDuration::from_millis(millis))
        .unwrap_or(Timestamp::UNIX_EPOCH);
    Now {
        instant: UnixMillis(timestamp.as_millisecond()),
        local: zone.to_datetime(timestamp),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::{civil::date, tz::TimeZone};

    use super::{UNIX_EPOCH, reading_at};
    use bunshin_core::{Now, UnixMillis};

    #[test]
    fn a_reading_before_the_epoch_keeps_the_epoch_fallback() {
        assert_eq!(
            reading_at(UNIX_EPOCH - Duration::from_secs(1), &TimeZone::UTC),
            Now {
                instant: UnixMillis(0),
                local: date(1970, 1, 1).at(0, 0, 0, 0),
            }
        );
    }

    #[test]
    fn both_views_come_from_one_millisecond_precision_sample() {
        assert_eq!(
            reading_at(
                UNIX_EPOCH + Duration::from_nanos(1_234_567_890),
                &TimeZone::fixed(jiff::tz::Offset::constant(9))
            ),
            Now {
                instant: UnixMillis(1_234),
                local: date(1970, 1, 1).at(9, 0, 1, 234_000_000),
            }
        );
    }

    #[test]
    fn a_sample_outside_the_civil_range_uses_the_epoch_fallback() {
        let year_ten_thousand = UNIX_EPOCH + Duration::from_secs(253_402_300_800);
        assert_eq!(
            reading_at(year_ten_thousand, &TimeZone::UTC),
            Now {
                instant: UnixMillis(0),
                local: date(1970, 1, 1).at(0, 0, 0, 0),
            }
        );
    }
}
