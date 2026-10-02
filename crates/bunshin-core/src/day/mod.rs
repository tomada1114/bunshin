//! A logical day's deterministic state and undoable task operations.
pub mod change;
pub mod file;
mod serde_civil;

use crate::{Tuning, UnixMillis};
pub use change::{Change, ChangeSet};
use jiff::civil::{Date, Time};
use serde::{Deserialize, Serialize};

/// The meaning of a task's optional civil time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskKind {
    /// No clock time.
    Untimed,
    /// Due by its clock time.
    Deadline,
    /// Begins at its clock time.
    Appointment,
}
/// A task's disposition on its own day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    /// Still to do.
    Open,
    /// Finished.
    Done,
    /// Explicitly abandoned for this day.
    Dropped,
    /// Copied to a later day by a separate carry-over use case.
    CarriedOver,
}
/// Where a task was first requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TaskOrigin {
    /// The owner's chat.
    Chat,
    /// A direct key action.
    Key,
    /// A leftover from another day.
    CarriedOver {
        /// The source logical date.
        #[serde(with = "serde_civil::date")]
        date: Date,
    },
}
/// One numbered task; mutation is only through a Day transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Never-reused identifier within its logical day.
    pub number: u64,
    /// Unicode title, validated against tuning when loaded or changed.
    pub title: String,
    /// Deadline, appointment, or untimed.
    pub kind: TaskKind,
    /// Minute-precision local time, present only for timed kinds.
    #[serde(with = "serde_civil::optional_time")]
    pub time: Option<Time>,
    /// Current disposition.
    pub status: TaskStatus,
    /// Supplied creation instant.
    pub created_at: UnixMillis,
    /// Closing instant, absent for an open task.
    pub closed_at: Option<UnixMillis>,
    /// Triggers already consumed, filled by the later check-in use case.
    pub triggers_fired: Vec<Trigger>,
    /// Original request source.
    pub origin: TaskOrigin,
}
/// Display facts shared by task rows and read-only subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskView {
    /// Identifier used by keys and plain-word references.
    pub number: u64,
    /// Validated title, formatted and truncated by the binary.
    pub title: String,
    /// Meaning of the optional clock time.
    pub kind: TaskKind,
    /// Minute-precision local time when timed.
    #[serde(with = "serde_civil::optional_time")]
    pub time: Option<Time>,
    /// Status determining the row's mark and grouping.
    pub status: TaskStatus,
    /// Source available to display and prompt callers.
    pub origin: TaskOrigin,
}

impl From<&Task> for TaskView {
    fn from(task: &Task) -> Self {
        Self {
            number: task.number,
            title: task.title.clone(),
            kind: task.kind,
            time: task.time,
            status: task.status,
            origin: task.origin.clone(),
        }
    }
}

/// Why a check-in was considered. Trigger rules are implemented separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TriggerKind {
    /// Before a deadline.
    BeforeDeadline,
    /// After a deadline.
    AfterDeadline,
    /// A model-planned look.
    PlannedLook,
    /// Beginning of a logical day.
    DayStart,
    /// Evening summary.
    EveningReview,
    /// Consolidated return from absence.
    CatchUp,
}
/// A considered trigger with its task and supplied instant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    /// Trigger reason.
    pub kind: TriggerKind,
    /// Task number when task-specific.
    pub task: Option<u64>,
    /// When it became due.
    pub due_at: UnixMillis,
}
/// The speaker of a chat row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Author {
    /// The owner.
    You,
    /// The secretary model.
    Bunshin,
    /// A deterministic app event.
    System,
}
/// The reason for a chat row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageKind {
    /// A prompted model response or owner's message.
    Reply,
    /// An unsolicited check-in.
    Unprompted,
    /// Structured task or mute changes, formatted by the binary.
    Change,
    /// An app notice.
    Notice,
    /// A recoverable app error.
    Error,
}
/// Note or question; unlike a question a note needs no reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnpromptedKind {
    /// Informational message.
    Note,
    /// Request for an answer.
    Question,
}
/// Stored inbox disposition; the later inbox use case owns transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InboxState {
    /// Awaiting reaction.
    Open,
    /// Explicit reply received.
    Answered,
    /// Note acknowledged.
    Acknowledged,
    /// Question dismissed.
    Dismissed,
    /// Its task closed.
    TaskClosed,
    /// Owner muted check-ins.
    Muted,
    /// No reaction within the reaction window.
    Ignored,
}
/// Why an unprompted message was suppressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuppressionReason {
    /// A recent check-in named the same task.
    SameTask,
}
/// Extra facts associated with an unprompted row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnpromptedMessage {
    /// Note or question.
    pub kind: UnpromptedKind,
    /// The triggering event.
    pub trigger: Trigger,
    /// Task being discussed, if any.
    pub task: Option<u64>,
    /// Current inbox disposition.
    pub inbox_state: InboxState,
    /// Last inbox transition instant.
    pub state_changed_at: UnixMillis,
    /// Suppression reason when no row was delivered.
    pub suppressed: Option<SuppressionReason>,
}
/// Stored chat data; change rows carry typed facts instead of user-facing wording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Speaker.
    pub author: Author,
    /// Owner/model text; empty for a structured change row.
    pub text: String,
    /// Supplied instant.
    pub time: UnixMillis,
    /// Row role.
    pub kind: MessageKind,
    /// Extra check-in facts.
    pub unprompted: Option<UnpromptedMessage>,
    /// Index of the question this message answers, if any.
    pub answers_question: Option<u64>,
    /// Typed visible change facts, including undo records.
    pub change_set: Option<ChangeSet>,
}
/// The previous day's record retained for context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YesterdayRecord {
    /// Logical date summarized.
    #[serde(with = "serde_civil::date")]
    pub date: Date,
    /// Short model summary supplied by the day-start use case.
    pub text: String,
}
/// A pure day's state, with a session-only undo stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Day {
    data: file::DayData,
    tuning: Tuning,
    undo: Vec<change::UndoEntry>,
}
/// Rejected day operation; no user data is carried in the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum DayError {
    /// A task title has zero characters.
    #[error("empty task title")]
    EmptyTitle,
    /// A title exceeds the configured character bound.
    #[error("task title too long")]
    TitleTooLong,
    /// The day's retained task count reached its configured limit.
    #[error("task limit reached")]
    LimitReached,
    /// Timed task has no time.
    #[error("timed task lacks time")]
    MissingTime,
    /// Untimed task unexpectedly carries a time.
    #[error("untimed task has time")]
    UnexpectedTime,
    /// Task time is not HH:MM precision.
    #[error("task time is not minute precision")]
    InvalidTime,
    /// Requested task does not exist.
    #[error("task not found")]
    TaskNotFound,
    /// The requested status transition is not available.
    #[error("invalid task status transition")]
    InvalidStatus,
    /// No retained session change remains to undo.
    #[error("nothing to undo")]
    NothingToUndo,
    /// All representable task numbers were consumed.
    #[error("task numbers exhausted")]
    NumberExhausted,
}

impl Day {
    /// An empty logical day, with no clock or storage read.
    #[must_use]
    pub fn new(date: Date, tuning: Tuning) -> Self {
        Self {
            data: file::DayData::empty(date),
            tuning,
            undo: Vec::new(),
        }
    }
    /// Logical date identifying this day.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.data.date
    }
    /// Retained tasks in creation order.
    #[must_use]
    pub fn tasks(&self) -> &[Task] {
        &self.data.tasks
    }
    /// Append-only chat rows, including visible changes and undo records.
    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.data.messages
    }
    /// Every persisted field, read-only; session undo is excluded.
    #[must_use]
    pub const fn data(&self) -> &file::DayData {
        &self.data
    }
    /// Owned rows in display order with task number resolving equal times.
    #[must_use]
    pub fn task_view(&self) -> Vec<TaskView> {
        let mut tasks = self.data.tasks.clone();
        tasks.sort_by_key(|task| {
            let group = match task.status {
                TaskStatus::Open => 0,
                TaskStatus::Done => 1,
                TaskStatus::Dropped => 2,
                TaskStatus::CarriedOver => 3,
            };
            let time = if task.status == TaskStatus::Open {
                task.time
            } else {
                None
            };
            (group, time.is_none(), time, task.number)
        });
        tasks.iter().map(TaskView::from).collect()
    }
    /// Add one open task and record exactly one change set.
    /// # Errors
    /// Invalid title/time, task limit, or exhausted identifiers.
    pub fn add(
        mut self,
        title: String,
        kind: TaskKind,
        time: Option<Time>,
        origin: TaskOrigin,
        at: UnixMillis,
    ) -> Result<(Self, ChangeSet), DayError> {
        validate_task(&title, kind, time, self.tuning)?;
        if self.data.tasks.len() >= self.tuning.day.tasks_per_day {
            return Err(DayError::LimitReached);
        }
        let number = self.data.next_task_number;
        let next = number.checked_add(1).ok_or(DayError::NumberExhausted)?;
        let before = self.snapshot();
        let task = Task {
            number,
            title,
            kind,
            time,
            status: TaskStatus::Open,
            created_at: at,
            closed_at: None,
            triggers_fired: Vec::new(),
            origin,
        };
        self.data.tasks.push(task.clone());
        self.data.next_task_number = next;
        Ok(self.record(
            before,
            vec![Change::Task {
                before: None,
                after: Some(task),
            }],
            at,
        ))
    }
    /// Edit only title, kind and time; metadata is retained.
    /// # Errors
    /// Invalid title/time or absent task.
    pub fn edit(
        mut self,
        number: u64,
        title: String,
        kind: TaskKind,
        time: Option<Time>,
        at: UnixMillis,
    ) -> Result<(Self, ChangeSet), DayError> {
        validate_task(&title, kind, time, self.tuning)?;
        let index = self.index(number)?;
        let snapshot = self.snapshot();
        let before = self.data.tasks[index].clone();
        let task = &mut self.data.tasks[index];
        task.title = title;
        task.kind = kind;
        task.time = time;
        let after = task.clone();
        Ok(self.record(
            snapshot,
            vec![Change::Task {
                before: Some(before),
                after: Some(after),
            }],
            at,
        ))
    }
    /// Close an open task as done.
    /// # Errors
    /// Absent task or task not open.
    pub fn done(self, number: u64, at: UnixMillis) -> Result<(Self, ChangeSet), DayError> {
        self.status(number, TaskStatus::Done, at)
    }
    /// Close an open task as dropped.
    /// # Errors
    /// Absent task or task not open.
    pub fn drop(self, number: u64, at: UnixMillis) -> Result<(Self, ChangeSet), DayError> {
        self.status(number, TaskStatus::Dropped, at)
    }
    /// Reopen a done or dropped task and clear its closing instant.
    /// # Errors
    /// Absent task, or task not done/dropped.
    pub fn reopen(self, number: u64, at: UnixMillis) -> Result<(Self, ChangeSet), DayError> {
        self.status(number, TaskStatus::Open, at)
    }
    /// Delete a task without making its identifier reusable.
    /// # Errors
    /// Task does not exist.
    pub fn delete(mut self, number: u64, at: UnixMillis) -> Result<(Self, ChangeSet), DayError> {
        let index = self.index(number)?;
        let snapshot = self.snapshot();
        let task = self.data.tasks.remove(index);
        Ok(self.record(
            snapshot,
            vec![Change::Task {
                before: Some(task),
                after: None,
            }],
            at,
        ))
    }
    /// Set a caller-computed mute end; duration decisions belong to later use cases.
    #[must_use]
    pub fn mute(mut self, until: UnixMillis, at: UnixMillis) -> (Self, ChangeSet) {
        let snapshot = self.snapshot();
        let before = self.data.muted_until;
        self.data.muted_until = Some(until);
        self.record(
            snapshot,
            vec![Change::Mute {
                before,
                after: Some(until),
            }],
            at,
        )
    }
    /// Clear mute, recording one undoable set.
    #[must_use]
    pub fn unmute(mut self, at: UnixMillis) -> (Self, ChangeSet) {
        let snapshot = self.snapshot();
        let before = self.data.muted_until;
        self.data.muted_until = None;
        self.record(
            snapshot,
            vec![Change::Mute {
                before,
                after: None,
            }],
            at,
        )
    }
    /// Walk back one session change, appending a visible undo row without undoing it.
    /// # Errors
    /// No retained session changes.
    pub fn undo(mut self, at: UnixMillis) -> Result<(Self, ChangeSet), DayError> {
        let entry = self.undo.pop().ok_or(DayError::NothingToUndo)?;
        self.data.tasks = entry.snapshot.tasks;
        self.data.muted_until = entry.snapshot.muted_until;
        let set = ChangeSet {
            time: at,
            changes: entry.changes,
            undo: true,
        };
        self.append_change(&set);
        Ok((self, set))
    }
    fn index(&self, number: u64) -> Result<usize, DayError> {
        self.data
            .tasks
            .iter()
            .position(|task| task.number == number)
            .ok_or(DayError::TaskNotFound)
    }
    fn status(
        mut self,
        number: u64,
        status: TaskStatus,
        at: UnixMillis,
    ) -> Result<(Self, ChangeSet), DayError> {
        let index = self.index(number)?;
        let before = self.data.tasks[index].clone();
        let valid = match status {
            TaskStatus::Open => matches!(before.status, TaskStatus::Done | TaskStatus::Dropped),
            TaskStatus::Done | TaskStatus::Dropped => before.status == TaskStatus::Open,
            TaskStatus::CarriedOver => false,
        };
        if !valid {
            return Err(DayError::InvalidStatus);
        }
        let snapshot = self.snapshot();
        self.data.tasks[index].status = status;
        self.data.tasks[index].closed_at = if status == TaskStatus::Open {
            None
        } else {
            Some(at)
        };
        let after = self.data.tasks[index].clone();
        Ok(self.record(
            snapshot,
            vec![Change::Task {
                before: Some(before),
                after: Some(after),
            }],
            at,
        ))
    }
    fn snapshot(&self) -> change::Snapshot {
        change::Snapshot {
            tasks: self.data.tasks.clone(),
            muted_until: self.data.muted_until,
        }
    }
    fn record(
        mut self,
        snapshot: change::Snapshot,
        changes: Vec<Change>,
        at: UnixMillis,
    ) -> (Self, ChangeSet) {
        self.undo.push(change::UndoEntry {
            snapshot,
            changes: changes.clone(),
        });
        if self.undo.len() > self.tuning.day.undo_depth {
            self.undo.remove(0);
        }
        let set = ChangeSet {
            time: at,
            changes,
            undo: false,
        };
        self.append_change(&set);
        (self, set)
    }
    fn append_change(&mut self, set: &ChangeSet) {
        self.data.messages.push(Message {
            author: Author::System,
            text: String::new(),
            time: set.time,
            kind: MessageKind::Change,
            unprompted: None,
            answers_question: None,
            change_set: Some(set.clone()),
        });
    }
}
fn validate_task(
    title: &str,
    kind: TaskKind,
    time: Option<Time>,
    tuning: Tuning,
) -> Result<(), DayError> {
    if title.is_empty() {
        return Err(DayError::EmptyTitle);
    }
    if title.chars().count() > tuning.day.title_max_chars {
        return Err(DayError::TitleTooLong);
    }
    match kind {
        TaskKind::Untimed if time.is_some() => return Err(DayError::UnexpectedTime),
        TaskKind::Deadline | TaskKind::Appointment if time.is_none() => {
            return Err(DayError::MissingTime);
        }
        TaskKind::Untimed | TaskKind::Deadline | TaskKind::Appointment => {}
    }
    if time.is_some_and(|time| time.second() != 0 || time.subsec_nanosecond() != 0) {
        return Err(DayError::InvalidTime);
    }
    Ok(())
}
