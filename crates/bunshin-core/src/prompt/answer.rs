//! Schema validation precedes all domain transitions; proposals share one undo.
use crate::{
    Tuning, UnixMillis,
    day::{Change, ChangeSet, Day, DayError, TaskKind, TaskOrigin},
    model::{ModelAnswer, ModelError},
};
use jiff::civil::Time;
use serde::{Deserialize, Deserializer, Serialize};

/// Which required domain input was absent from a structurally valid proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalField {
    /// Task identifier.
    Task,
    /// Task title.
    Title,
    /// Task time kind.
    Kind,
    /// Mute duration in minutes.
    Minutes,
}
/// Refusals carry only actionable kinds, never the model's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum RefusalReason {
    /// An operation lacks its domain input.
    #[error("proposal lacks a required field")]
    MissingField {
        /// Absent input.
        field: ProposalField,
    },
    /// Time was not an explicit, valid HH:MM string.
    #[error("proposal time is not HH:MM")]
    InvalidTime,
    /// Mute duration is outside tuning's inclusive interval.
    #[error("proposal mute duration is outside the interval")]
    MuteOutOfRange,
    /// The supplied instant cannot represent the mute end.
    #[error("proposal mute end is not representable")]
    MuteOverflow,
    /// The existing day rules rejected this operation.
    #[error("proposal violates a day rule")]
    Domain {
        /// The day rule.
        reason: DayError,
    },
}
impl From<DayError> for RefusalReason {
    fn from(reason: DayError) -> Self {
        Self::Domain { reason }
    }
}
/// One dropped proposal, in the model's original array order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusedChange {
    /// Zero-based index in `changes`.
    pub index: usize,
    /// Reason for the binary to format beside the reply.
    pub reason: RefusalReason,
}
/// A validated answer, ready for a caller to save and display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatOutcome {
    /// New day, with accepted operations as one session undo entry.
    pub day: Day,
    /// One visible change row, absent when nothing changed.
    pub change_set: Option<ChangeSet>,
    /// Complete reply, including text longer than the requested bound.
    pub reply: String,
    /// Domain-invalid operations, in original order.
    pub refused: Vec<RefusedChange>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    changes: Vec<Proposal>,
    reply: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Operation {
    Add,
    Done,
    Drop,
    Reopen,
    ChangeTime,
    Rename,
    Mute,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    op: Operation,
    #[serde(default, deserialize_with = "present")]
    task: Option<i128>,
    #[serde(default, deserialize_with = "present")]
    title: Option<String>,
    #[serde(default, deserialize_with = "present")]
    kind: Option<TaskKind>,
    #[serde(default, deserialize_with = "present")]
    time: Option<String>,
    #[serde(default, deserialize_with = "present")]
    minutes: Option<i128>,
}
// A schema-optional field may be omitted, but its declared type excludes null.
fn present<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
fn required<T>(value: Option<T>, field: ProposalField) -> Result<T, RefusalReason> {
    value.ok_or(RefusalReason::MissingField { field })
}
fn number(value: Option<i128>) -> Result<u64, RefusalReason> {
    u64::try_from(required(value, ProposalField::Task)?).map_err(|_| DayError::TaskNotFound.into())
}
fn clock_time(text: Option<&str>) -> Result<Option<Time>, RefusalReason> {
    let Some(text) = text else {
        return Ok(None);
    };
    let bytes = text.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes[..2].iter().chain(&bytes[3..]).all(u8::is_ascii_digit)
    {
        return Err(RefusalReason::InvalidTime);
    }
    let hour = (bytes[0] - b'0') * 10 + bytes[1] - b'0';
    let minute = (bytes[3] - b'0') * 10 + bytes[4] - b'0';
    Time::new(
        i8::try_from(hour).map_err(|_| RefusalReason::InvalidTime)?,
        i8::try_from(minute).map_err(|_| RefusalReason::InvalidTime)?,
        0,
        0,
    )
    .map(Some)
    .map_err(|_| RefusalReason::InvalidTime)
}
impl Proposal {
    fn apply(
        self,
        day: Day,
        at: UnixMillis,
        tuning: Tuning,
    ) -> Result<(Day, ChangeSet), RefusalReason> {
        match self.op {
            Operation::Add => {
                let title = required(self.title, ProposalField::Title)?;
                let kind = required(self.kind, ProposalField::Kind)?;
                let time = clock_time(self.time.as_deref())?;
                day.add(title, kind, time, TaskOrigin::Chat, at)
                    .map_err(Into::into)
            }
            Operation::Done => day.done(number(self.task)?, at).map_err(Into::into),
            Operation::Drop => day.drop(number(self.task)?, at).map_err(Into::into),
            Operation::Reopen => day.reopen(number(self.task)?, at).map_err(Into::into),
            Operation::ChangeTime => {
                let kind = required(self.kind, ProposalField::Kind)?;
                let time = clock_time(self.time.as_deref())?;
                edit(day, number(self.task)?, None, Some((kind, time)), at)
            }
            Operation::Rename => edit(
                day,
                number(self.task)?,
                Some(required(self.title, ProposalField::Title)?),
                None,
                at,
            ),
            Operation::Mute => {
                let minutes = required(self.minutes, ProposalField::Minutes)?;
                let min = i128::from(tuning.checkin.chat_mute_min_minutes);
                let max = i128::from(tuning.checkin.chat_mute_max_minutes);
                if minutes < min || minutes > max {
                    return Err(RefusalReason::MuteOutOfRange);
                }
                let millis =
                    i64::try_from(minutes * 60_000).map_err(|_| RefusalReason::MuteOverflow)?;
                let until =
                    at.0.checked_add(millis)
                        .ok_or(RefusalReason::MuteOverflow)?;
                Ok(day.mute(UnixMillis(until), at))
            }
        }
    }
}
fn edit(
    day: Day,
    number: u64,
    title: Option<String>,
    schedule: Option<(TaskKind, Option<Time>)>,
    at: UnixMillis,
) -> Result<(Day, ChangeSet), RefusalReason> {
    let task = day
        .tasks()
        .iter()
        .find(|task| task.number == number)
        .ok_or(DayError::TaskNotFound)?;
    let title = title.unwrap_or_else(|| task.title.clone());
    let (kind, time) = schedule.unwrap_or((task.kind, task.time));
    day.edit(number, title, kind, time, at).map_err(Into::into)
}
/// Parse the entire schema before touching state, then accept domain-valid
/// proposals in order. The borrowed input day always remains unchanged.
///
/// # Errors
/// `Malformed` for invalid JSON, envelope, operation or field types. No changes
/// are returned or applied when any structural failure occurs.
pub fn apply_chat(
    day: &Day,
    answer: &ModelAnswer,
    at: UnixMillis,
    tuning: Tuning,
) -> Result<ChatOutcome, ModelError> {
    let envelope: Envelope =
        serde_json::from_str(&answer.json).map_err(|_| ModelError::Malformed)?;
    let mut next = day.clone();
    let mut changes = Vec::new();
    let mut refused = Vec::new();
    for (index, proposal) in envelope.changes.into_iter().enumerate() {
        match proposal.apply(next.clone(), at, tuning) {
            Ok((candidate, set)) => {
                let facts = set
                    .changes
                    .into_iter()
                    .filter(|change| match change {
                        Change::Task { before, after } => before != after,
                        Change::Mute { before, after } => before != after,
                    })
                    .collect::<Vec<_>>();
                if !facts.is_empty() {
                    next = candidate;
                    changes.extend(facts);
                }
            }
            Err(reason) => refused.push(RefusedChange { index, reason }),
        }
    }
    let (day, change_set) = next.group_changes(day, changes, at);
    Ok(ChatOutcome {
        day,
        change_set,
        reply: envelope.reply,
        refused,
    })
}
