//! The clock contract, shared by the fake and the OS adapter.
use bunshin_test_support::FixedClock;
use bunshin_test_support::clock_contract;

#[test]
fn fixed_clock_meets_the_clock_contract() {
    clock_contract(|| Box::new(FixedClock::default()));
}
