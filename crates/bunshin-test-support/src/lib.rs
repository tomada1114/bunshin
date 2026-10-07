//! Fakes for every `bunshin-core` port, and one contract function per port.
//!
//! A contract function holds the behaviour every implementation of a port must have.
//! `bunshin-core`'s integration tests run it against the fake here; `bunshin-platform`'s
//! tests run the same function against the real adapter — so the fake cannot drift
//! from the real thing without a test failing.
//!
//! This crate is a `[dev-dependencies]` entry only; a harness check fails if a normal
//! dependency edge points at it. Its functions are library code, not tests, so they
//! compare `Result`s with `assert_eq!` instead of unwrapping.

mod clock;
mod day_store;
mod language_model;

pub use clock::{FixedClock, clock_contract};
pub use day_store::{
    FailingDayStore, InMemoryDayStore, day_store_contract, day_store_refusal_contract,
};
pub use language_model::{ScriptedLanguageModel, language_model_contract};
