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
pub mod checkin;
pub mod day;
pub mod inbox;
pub mod instructions;
pub mod model;
pub mod prompt;
pub mod rhythm;
pub mod screen;
pub mod shell;
pub mod time;
pub mod tuning;
pub use tuning::{CheckinTuning, DayTuning, PromptTuning, RhythmTuning};

pub use shell::{ShellAction, ShellKey, ShellScreen};
pub use time::{Clock, Now, UnixMillis, logical_date};

pub use model::{
    Availability, CancelFlag, LanguageModel, ModelAnswer, ModelError, ModelRequest,
    UnavailableReason,
};
pub use tuning::Tuning;
