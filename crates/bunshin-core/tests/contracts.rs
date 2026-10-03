//! The clock contract, shared by the fake and the OS adapter.
use bunshin_test_support::FixedClock;
use bunshin_test_support::clock_contract;

#[test]
fn fixed_clock_meets_the_clock_contract() {
    clock_contract(|| Box::new(FixedClock::default()));
}

#[test]
fn scripted_model_meets_contract() {
    bunshin_test_support::language_model_contract(|| {
        Box::new(bunshin_test_support::ScriptedLanguageModel::new([Ok(
            bunshin_core::ModelAnswer { json: "{}".into() },
        )]))
    });
}

#[test]
fn in_memory_day_store_meets_contract() {
    bunshin_test_support::day_store_contract(|| {
        Box::new(bunshin_test_support::InMemoryDayStore::default())
    });
}

#[test]
fn failing_day_store_returns_unavailable_without_changing_a_day() {
    use bunshin_core::day::{
        Day,
        store::{DayStore, DayStoreError},
    };
    let store = bunshin_test_support::FailingDayStore;
    let date = jiff::civil::date(2026, 10, 2);
    let day = Day::new(date, bunshin_core::Tuning::default());
    assert_eq!(store.load(date), Err(DayStoreError::Unavailable));
    assert_eq!(store.save(&day), Err(DayStoreError::Unavailable));
    assert_eq!(store.last_before(date), Err(DayStoreError::Unavailable));
    assert!(matches!(store.take_lock(), Err(DayStoreError::Unavailable)));
    assert!(day.tasks().is_empty());
}

#[test]
fn refused_in_memory_files_stay_refused_after_save_attempts() {
    use bunshin_core::day::store::DayStoreError;
    for error in [
        DayStoreError::Unreadable,
        DayStoreError::NewerFormat { found: 2 },
        DayStoreError::UnsupportedFormat { found: 0 },
    ] {
        let store = bunshin_test_support::InMemoryDayStore::default();
        let date = jiff::civil::date(2026, 10, 2);
        store.seed_error(date, error);
        bunshin_test_support::day_store_refusal_contract(&store, date, error);
    }
}

#[test]
#[should_panic(expected = "assertion")]
fn failing_day_store_intentionally_fails_success_contract() {
    bunshin_test_support::day_store_contract(|| Box::new(bunshin_test_support::FailingDayStore));
}
