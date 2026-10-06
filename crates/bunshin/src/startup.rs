//! Prepare data and the lifetime lease before the binary enters a terminal.

use bunshin_core::{
    Now, Tuning,
    day::store::{DayLock, DayStore, DayStoreError},
    logical_date,
    screen::MainScreen,
};

/// Acquire the lifetime lease before loading; a refusal drops it before any terminal entry.
/// # Errors
/// Propagates typed lock or day-load refusals without replacing the day file.
pub(crate) fn prepare(
    store: &dyn DayStore,
    now: Now,
    tuning: Tuning,
) -> Result<(Box<dyn DayLock>, MainScreen), DayStoreError> {
    let lease = store.take_lock()?;
    let day = store.load(logical_date(now.local, tuning.day_boundary))?;
    Ok((lease, MainScreen::new(day, tuning)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bunshin_core::{
        Clock, Tuning,
        day::store::{DayStore, DayStoreError},
        logical_date,
    };
    use bunshin_test_support::{FixedClock, InMemoryDayStore};

    #[test]
    fn another_writer_is_refused_before_loading_an_unreadable_day() {
        let tuning = Tuning::default();
        let clock = FixedClock::default();
        let date = logical_date(clock.now().local, tuning.day_boundary);
        let store = InMemoryDayStore::new(tuning);
        store.seed_error(date, DayStoreError::Unreadable);
        let _lease = store.take_lock().expect("other writer");
        assert_eq!(
            prepare(&store, clock.now(), tuning).map(|_| ()),
            Err(DayStoreError::AlreadyLocked { pid: Some(1) })
        );
    }

    #[test]
    fn refused_day_data_releases_the_startup_lease_without_creating_a_day() {
        for error in [
            DayStoreError::Unreadable,
            DayStoreError::NewerFormat { found: 2 },
        ] {
            let tuning = Tuning::default();
            let clock = FixedClock::default();
            let date = logical_date(clock.now().local, tuning.day_boundary);
            let store = InMemoryDayStore::new(tuning);
            store.seed_error(date, error);
            assert_eq!(prepare(&store, clock.now(), tuning).map(|_| ()), Err(error));
            assert_eq!(store.load(date).map(|_| ()), Err(error));
            let _lease = store.take_lock().expect("startup lease released");
        }
    }

    #[test]
    fn opening_a_missing_day_loads_an_empty_screen_and_keeps_the_lease() {
        let tuning = Tuning::default();
        let clock = FixedClock::default();
        let store = InMemoryDayStore::new(tuning);
        let (_lease, screen) = prepare(&store, clock.now(), tuning).expect("start");
        assert_eq!(
            screen.day().date(),
            logical_date(clock.now().local, tuning.day_boundary)
        );
        assert!(screen.day().tasks().is_empty());
        assert!(screen.day().messages().is_empty());
        assert!(matches!(
            store.take_lock(),
            Err(DayStoreError::AlreadyLocked { .. })
        ));
    }

    #[test]
    fn real_store_lock_and_invalid_data_are_refused_without_changing_day_bytes() {
        use bunshin_platform::JsonFileDayStore;
        use std::fs;
        let tuning = Tuning::default();
        let now = FixedClock::default().now();
        let scratch = tempfile::tempdir().expect("scratch");
        let root = scratch.path().join("private-owner-data");
        let store = JsonFileDayStore::new(root.clone(), tuning);
        let lease = store.take_lock().expect("other writer");
        let date = logical_date(now.local, tuning.day_boundary);
        let days = root.join("days");
        assert!(
            days.is_dir(),
            "taking the real lease creates the private days directory"
        );
        let path = days.join(format!("{date}.json"));
        fs::write(&path, b"invalid JSON").expect("invalid fixture");
        assert_eq!(
            prepare(&store, now, tuning).map(|_| ()),
            Err(DayStoreError::AlreadyLocked {
                pid: Some(std::process::id())
            })
        );
        assert_eq!(fs::read(&path).expect("unchanged"), b"invalid JSON");
        drop(lease);
        for (bytes, error) in [
            (b"invalid JSON".as_slice(), DayStoreError::Unreadable),
            (
                br#"{"format":3}"#.as_slice(),
                DayStoreError::NewerFormat { found: 3 },
            ),
        ] {
            fs::write(&path, bytes).expect("fixture");
            assert_eq!(prepare(&store, now, tuning).map(|_| ()), Err(error));
            assert_eq!(fs::read(&path).expect("unchanged"), bytes);
            let lease = store.take_lock().expect("startup released lease");
            drop(lease);
        }
    }
}
