//! The app's domain logic.
//!
//! Core holds the rules and the state; it never touches the operating system. Time,
//! storage, and anything else outside the process reach it through *ports* — traits
//! declared here and implemented by `bunshin-platform` (the real thing) or
//! `bunshin-test-support` (fakes). See `docs/architecture.md`.

// Every `match` on a core enum names each variant, so adding a variant is a compile
// error at every place that must decide what it means (`.claude/rules/testing.md`).
#![deny(clippy::wildcard_enum_match_arm)]

pub mod board;
pub mod day;
pub mod model;
pub mod prompt;
pub mod screen;
pub mod time;
pub mod tuning;
pub use board::BoardTuning;
pub use time::{Clock, Now, UnixMillis, logical_date};
pub use tuning::{ChatTuning, DayTuning, PromptTuning};

pub use model::{
    Availability, CancelFlag, LanguageModel, ModelAnswer, ModelError, ModelRequest,
    UnavailableReason,
};
pub use tuning::Tuning;
