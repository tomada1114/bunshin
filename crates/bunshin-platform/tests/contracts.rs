//! The port contracts from bunshin-test-support, run against the real adapters. Each test
//! gets its own temporary directory; nothing touches the real `~/Library`.

use bunshin_platform::{JsonFileCounterStore, SystemClock};
use bunshin_test_support::{clock_contract, counter_store_contract};

#[test]
fn json_file_store_meets_the_counter_store_contract() {
    let mut dirs = Vec::new(); // keeps every TempDir alive until the test ends
    counter_store_contract(|| {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileCounterStore::new(dir.path().join("nested").join("counter.json"));
        dirs.push(dir);
        Box::new(store)
    });
}

#[test]
fn system_clock_meets_the_clock_contract() {
    clock_contract(|| Box::new(SystemClock));
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
