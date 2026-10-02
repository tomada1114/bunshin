//! Clock readings keep instants and civil time separate, even across a zone change.

use bunshin_core::{Clock, Now, UnixMillis};
use bunshin_test_support::{FixedClock, clock_contract};
use jiff::{Timestamp, civil::date, tz::Offset};

#[test]
fn the_default_clock_returns_both_views_of_one_instant() {
    assert_eq!(
        FixedClock::default().now(),
        Now {
            instant: UnixMillis(1_700_000_000_000),
            local: date(2023, 11, 14).at(22, 13, 20, 0),
        }
    );
}

#[test]
fn a_fixed_clock_can_start_at_a_chosen_offset() {
    let clock = FixedClock::at(UnixMillis(1_790_899_200_000), Offset::constant(9)).unwrap();
    assert_eq!(
        clock.now(),
        Now {
            instant: UnixMillis(1_790_899_200_000),
            local: date(2026, 10, 2).at(9, 0, 0, 0),
        }
    );
    clock_contract(|| {
        Box::new(FixedClock::at(UnixMillis(1_790_899_200_000), Offset::constant(9)).unwrap())
    });
}

#[test]
fn advancing_a_fixed_clock_moves_both_views_together() {
    let clock = FixedClock::at(UnixMillis(1_790_899_200_000), Offset::constant(9)).unwrap();
    clock.advance(5_400_000).unwrap();
    assert_eq!(
        clock.now(),
        Now {
            instant: UnixMillis(1_790_904_600_000),
            local: date(2026, 10, 2).at(10, 30, 0, 0),
        }
    );
}

#[test]
fn setting_an_offset_changes_civil_time_without_changing_the_instant() {
    let clock = FixedClock::default();
    clock
        .set(UnixMillis(1_790_899_200_000), Offset::constant(9))
        .unwrap();
    let tokyo = clock.now();
    clock.set(tokyo.instant, Offset::UTC).unwrap();
    assert_eq!(
        clock.now(),
        Now {
            instant: tokyo.instant,
            local: date(2026, 10, 2).at(0, 0, 0, 0),
        }
    );
}

#[test]
fn a_fixed_clock_can_move_backwards_across_midnight() {
    let clock = FixedClock::at(UnixMillis(0), Offset::UTC).unwrap();
    clock.advance(-1).unwrap();
    assert_eq!(
        clock.now(),
        Now {
            instant: UnixMillis(-1),
            local: date(1969, 12, 31).at(23, 59, 59, 999_000_000),
        }
    );
}

#[test]
fn the_clock_contract_accepts_early_dates_and_extreme_offsets() {
    for (instant, offset) in [
        (UnixMillis(0), Offset::UTC),
        (UnixMillis(-1), Offset::UTC),
        (UnixMillis(-1), Offset::from_seconds(30).unwrap()),
        (UnixMillis(Timestamp::MIN.as_millisecond()), Offset::MIN),
        (UnixMillis(Timestamp::MAX.as_millisecond()), Offset::MAX),
    ] {
        clock_contract(|| Box::new(FixedClock::at(instant, offset).unwrap()));
    }
}

#[test]
fn an_unrepresentable_instant_is_refused_without_changing_the_clock() {
    let clock = FixedClock::default();
    let before = clock.now();
    assert!(clock.set(UnixMillis(i64::MAX), Offset::UTC).is_err());
    assert_eq!(clock.now(), before);
    assert!(clock.set(UnixMillis(i64::MIN), Offset::UTC).is_err());
    assert_eq!(clock.now(), before);
    assert!(FixedClock::at(UnixMillis(i64::MAX), Offset::UTC).is_err());
}

#[test]
fn an_unrepresentable_advance_leaves_both_views_unchanged() {
    let clock = FixedClock::default();
    let before = clock.now();
    assert!(clock.advance(i64::MAX).is_err());
    assert_eq!(clock.now(), before);
    assert!(clock.advance(i64::MIN).is_err());
    assert_eq!(clock.now(), before);
}
