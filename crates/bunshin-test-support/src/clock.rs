use std::sync::Mutex;
use std::sync::PoisonError;

use bunshin_core::{Clock, Now, UnixMillis};
use jiff::{
    SignedDuration, Timestamp, Zoned,
    civil::date,
    tz::{Offset, TimeZone},
};

/// A clock that returns whatever it was set to. Tests move time with [`FixedClock::set`]
/// or [`FixedClock::advance`] instead of sleeping.
#[derive(Debug)]
pub struct FixedClock {
    now: Mutex<Zoned>,
}

impl FixedClock {
    /// 2023-11-14T22:13:20Z — a fixed, recognisable instant.
    pub const DEFAULT: UnixMillis = UnixMillis(1_700_000_000_000);

    /// A clock stopped at `instant` in a fixed UTC offset, independent of the host zone.
    ///
    /// # Errors
    /// When `instant` is outside jiff's representable timestamp range.
    pub fn at(instant: UnixMillis, offset: Offset) -> Result<Self, jiff::Error> {
        let now = Timestamp::from_duration(SignedDuration::from_millis(instant.0))?
            .to_zoned(TimeZone::fixed(offset));
        Ok(Self {
            now: Mutex::new(now),
        })
    }

    /// Move both views to `instant` with `offset`; an error leaves the clock unchanged.
    ///
    /// # Errors
    /// When `instant` is outside jiff's representable timestamp range.
    pub fn set(&self, instant: UnixMillis, offset: Offset) -> Result<(), jiff::Error> {
        let now = Timestamp::from_duration(SignedDuration::from_millis(instant.0))?
            .to_zoned(TimeZone::fixed(offset));
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = now;
        Ok(())
    }

    /// Move both views by `millis`, which may be negative, without changing the offset.
    ///
    /// # Errors
    /// When the result is outside jiff's representable timestamp range. Neither view
    /// changes on error.
    pub fn advance(&self, millis: i64) -> Result<(), jiff::Error> {
        let mut now = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        *now = now.checked_add(SignedDuration::from_millis(millis))?;
        Ok(())
    }
}

impl Default for FixedClock {
    fn default() -> Self {
        Self {
            now: Mutex::new(
                Timestamp::constant(Self::DEFAULT.0 / 1_000, 0).to_zoned(TimeZone::UTC),
            ),
        }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Now {
        let now = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        Now {
            instant: UnixMillis(now.timestamp().as_millisecond()),
            local: now.datetime(),
        }
    }
}

/// Every [`Clock`] supplies an instant and civil time at the same millisecond precision,
/// related by a valid UTC offset. No particular date or zone is required.
///
/// It asserts no order between two reads: [`Clock`] is a wall clock, and the real one
/// (`SystemClock`, over `SystemTime::now`) steps backwards when the system time is
/// corrected, so core must not rely on time only moving forward.
///
/// # Panics
/// When the clock breaks the contract; that is how the calling test fails.
pub fn clock_contract(mut make: impl FnMut() -> Box<dyn Clock>) {
    let read = make().now();
    let civil_at_utc = read
        .local
        .duration_since(date(1970, 1, 1).at(0, 0, 0, 0))
        .as_millis();
    let offset_millis = civil_at_utc - i128::from(read.instant.0);
    let minimum_offset = i128::from(Offset::MIN.seconds()) * 1_000;
    let maximum_offset = i128::from(Offset::MAX.seconds()) * 1_000;
    assert!(
        (minimum_offset..=maximum_offset).contains(&offset_millis),
        "the two views must differ by a valid UTC offset: {read:?}"
    );
    assert_eq!(
        offset_millis % 1_000,
        0,
        "a UTC offset has whole seconds: {read:?}"
    );
    assert_eq!(
        i64::from(read.local.subsec_nanosecond()),
        read.instant.0.rem_euclid(1_000) * 1_000_000,
        "both views must describe the same millisecond: {read:?}"
    );
}
