//! The clock contract, shared by the fake and the OS adapter.
use bunshin_platform::SystemClock;
use bunshin_test_support::clock_contract;

#[test]
fn system_clock_meets_the_clock_contract() {
    clock_contract(|| Box::new(SystemClock));
}

#[test]
fn json_file_day_store_meets_contract() {
    let scratch = tempfile::tempdir().expect("scratch directory");
    let mut count = 0;
    bunshin_test_support::day_store_contract(|| {
        count += 1;
        Box::new(bunshin_platform::JsonFileDayStore::new(
            scratch.path().join(count.to_string()),
            bunshin_core::Tuning::default(),
        ))
    });
}

#[test]
fn unsupported_model_meets_contract() {
    bunshin_test_support::language_model_contract(|| {
        Box::new(bunshin_platform::UnavailableLanguageModel)
    });
}
#[cfg(target_os = "macos")]
#[test]
#[ignore = "local machine: fm with Apple Intelligence enabled"]
fn real_fm_meets_contract() {
    bunshin_test_support::language_model_contract(|| {
        Box::new(bunshin_platform::FmLanguageModel::default())
    });
}
