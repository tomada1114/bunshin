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
