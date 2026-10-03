//! Strict check-in envelopes permit no task mutations.
use crate::{ModelError, Tuning, day::Day};
use serde::{Deserialize, Deserializer};

/// Whether the model asks to deliver a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckinKind {
    /// Plan only; no message or bell.
    Silent,
    /// Informational row.
    Note,
    /// Row expecting an owner reply.
    Question,
}
/// Domain-validated answer; private message text is not included in Debug.
#[derive(Clone, PartialEq, Eq)]
pub struct CheckinAnswer {
    /// Delivery choice.
    pub kind: CheckinKind,
    /// Known task reference, absent for general or unknown references.
    pub task: Option<u64>,
    /// Complete model text; the model's requested length is advisory.
    pub message: String,
    /// Clamped delay including the configured missing-value default.
    pub next_look_minutes: u16,
}
impl std::fmt::Debug for CheckinAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckinAnswer")
            .field("kind", &self.kind)
            .field("task", &self.task)
            .field("next_look_minutes", &self.next_look_minutes)
            .finish_non_exhaustive()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    kind: CheckinKind,
    message: String,
    #[serde(default, deserialize_with = "present")]
    task: Option<i128>,
    #[serde(default, deserialize_with = "present")]
    next_look_minutes: Option<i128>,
}
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
/// Parse the entire envelope before delivery. Unknown tasks are general notes;
/// invalid types, nulls, extra fields and missing required fields are malformed.
/// # Errors
/// Returns `ModelError::Malformed` without any model text.
pub fn parse_checkin(json: &str, day: &Day, tuning: Tuning) -> Result<CheckinAnswer, ModelError> {
    let raw: Envelope = serde_json::from_str(json).map_err(|_| ModelError::Malformed)?;
    let bounds = tuning.checkin;
    let delay = raw
        .next_look_minutes
        .unwrap_or(i128::from(bounds.planned_default_minutes))
        .max(i128::from(bounds.planned_min_minutes))
        .min(i128::from(
            bounds.planned_max_minutes.max(bounds.planned_min_minutes),
        ));
    let next_look_minutes = u16::try_from(delay).map_err(|_| ModelError::Malformed)?;
    Ok(CheckinAnswer {
        kind: raw.kind,
        task: raw
            .task
            .and_then(|number| u64::try_from(number).ok())
            .filter(|number| day.tasks().iter().any(|task| task.number == *number)),
        message: raw.message,
        next_look_minutes,
    })
}
