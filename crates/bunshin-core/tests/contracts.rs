//! The port contracts from bunshin-test-support, run against its fakes. The same functions
//! run against the real adapters in bunshin-platform's tests.

use bunshin_test_support::{
    FixedClock, InMemoryCounterStore, clock_contract, counter_store_contract,
};

#[test]
fn in_memory_store_meets_the_counter_store_contract() {
    counter_store_contract(|| Box::new(InMemoryCounterStore::default()));
}

#[test]
fn fixed_clock_meets_the_clock_contract() {
    clock_contract(|| Box::new(FixedClock::default()));
}
