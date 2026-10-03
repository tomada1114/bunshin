//! A fresh owner-chat request with a conservative bound over the complete session.
use super::{budget::estimate, rules::operating_rules};
use crate::{
    ModelRequest, Now, Tuning,
    day::{
        Author, Day, InboxState, MessageKind, TaskKind, TaskStatus, TaskView, Trigger,
        UnpromptedKind,
    },
    instructions::InstructionsState,
};
use serde::Serialize;

const MODEL_WINDOW: usize = 4096;
/// Generation schema; domain validation separately rejects invalid individual changes.
/// `fm` object names and field order follow its generated schema format. Reply
/// generation comes first; changes-first repeatedly stalled the empty-change
/// synthetic request on the local model (observed 2026-10-03).
pub const CHAT_SCHEMA: &str = r##"{"type":"object","properties":{"changes":{"type":"array","items":{"$ref":"#/$defs/Change"}},"reply":{"type":"string"}},"required":["changes","reply"],"additionalProperties":false,"title":"ChatAnswer","x-order":["reply","changes"],"$defs":{"Change":{"type":"object","properties":{"op":{"type":"string","enum":["add","done","drop","reopen","changeTime","rename","mute"]},"task":{"type":"integer"},"title":{"type":"string"},"kind":{"type":"string","enum":["untimed","deadline","appointment"]},"time":{"type":"string"},"minutes":{"type":"integer"}},"required":["op"],"additionalProperties":false,"title":"Change","x-order":["op","task","title","kind","time","minutes"]}}}"##;

/// A future inbox caller supplies state facts rather than message text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnpromptedContext {
    /// The referenced task, absent for a general note.
    pub task: Option<u64>,
    /// Whether a response was expected.
    pub kind: UnpromptedKind,
    /// Owner reaction, without private message text.
    pub state: InboxState,
}
/// Explicit slots for later day-start, inbox and check-in callers; empty in chat's first slice.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextExtras<'a> {
    /// Deterministic previous-day record, capped by its estimated token bound.
    pub yesterday: Option<&'a str>,
    /// States in chronological order; at most the configured newest states are included.
    pub unprompted_states: &'a [UnpromptedContext],
    /// A caller's trigger facts, included as a whole block when they fit.
    pub triggers: &'a [Trigger],
}
/// Numeric evidence for the request actually returned, without owner text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetReport {
    /// Instructions + prompt + schema + answer reserve.
    pub estimated_tokens: usize,
    /// Effective bound, never above the model's window.
    pub limit: usize,
    /// Every open task stays in the request.
    pub open_tasks: usize,
    /// Oldest conversational rows removed before other reductions.
    pub dropped_chat: usize,
    /// Closed task rows removed after conversational history.
    pub dropped_closed_tasks: usize,
    /// Open titles shortened in request copies only.
    pub shortened_titles: usize,
}
/// The request and its aggregate evidence; Debug inherits `ModelRequest`'s text redaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltChat {
    /// A single fresh model call.
    pub request: ModelRequest,
    /// Conservative budget for this exact request.
    pub budget: BudgetReport,
}
/// Assembly refuses before a caller starts the model; no variant contains user text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum PromptError {
    /// Owner message exceeded the Unicode-scalar input bound.
    #[error("owner message exceeds character limit")]
    InputTooLong {
        /// Actual scalar count.
        chars: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// Even mandatory instructions, current input and minimal task rows cannot fit.
    #[error("required context exceeds model window")]
    RequiredContextTooLong,
    /// A context value could not be encoded, without exposing serialization diagnostics.
    #[error("prompt context encoding failed")]
    EncodingFailed,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptTask {
    number: u64,
    title: String,
    kind: TaskKind,
    time: Option<String>,
    // The openTasks block already states this fact; retain status only on closed rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<TaskStatus>,
}
impl From<TaskView> for PromptTask {
    fn from(task: TaskView) -> Self {
        Self {
            number: task.number,
            title: task.title,
            kind: task.kind,
            time: task
                .time
                .map(|time| format!("{:02}:{:02}", time.hour(), time.minute())),
            status: match task.status {
                TaskStatus::Open => None,
                TaskStatus::Done | TaskStatus::Dropped | TaskStatus::CarriedOver => {
                    Some(task.status)
                }
            },
        }
    }
}
#[derive(Clone, Serialize)]
struct HistoryRow {
    author: Author,
    text: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Context {
    date: String,
    now: String,
    yesterday: Option<String>,
    open_tasks: Vec<PromptTask>,
    unprompted_states: Vec<UnpromptedContext>,
    triggers: Vec<Trigger>,
    closed_tasks: Vec<PromptTask>,
    chat: Vec<HistoryRow>,
    message: String,
}
fn encode(context: &Context) -> Result<String, PromptError> {
    serde_json::to_string(context).map_err(|_| PromptError::EncodingFailed)
}
fn total(instructions: &str, prompt: &str, tuning: Tuning) -> usize {
    let ratio = tuning.prompt.ascii_chars_per_token;
    estimate(instructions, ratio)
        .saturating_add(estimate(prompt, ratio))
        .saturating_add(estimate(CHAT_SCHEMA, ratio))
        .saturating_add(tuning.prompt.chat_answer_tokens)
}
/// Build one bounded request. Drop oldest chat, then closed tasks, then shorten open
/// titles in copies. Current input and every open identifier/time are mandatory.
/// A matching final owner row at this supplied instant is the current message,
/// so it is included in `message` only, rather than duplicated in history.
/// # Errors
/// `InputTooLong` before assembly; `RequiredContextTooLong` if mandatory content cannot
/// fit even with minimal titles; `EncodingFailed` for context serialization failure.
pub fn build_chat(
    day: &Day,
    owner: &InstructionsState,
    input: &str,
    now: Now,
    extras: ContextExtras<'_>,
    tuning: Tuning,
) -> Result<BuiltChat, PromptError> {
    let chars = input.chars().count();
    if chars > tuning.prompt.input_max_chars {
        return Err(PromptError::InputTooLong {
            chars,
            limit: tuning.prompt.input_max_chars,
        });
    }
    let instructions = format!("{}{}", owner.text, operating_rules(tuning));
    let limit = tuning.prompt.context_tokens.min(MODEL_WINDOW);
    let tasks = day.task_view();
    let mut context = Context {
        date: day.date().to_string(),
        now: now.local.to_string(),
        yesterday: None,
        open_tasks: tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Open)
            .cloned()
            .map(PromptTask::from)
            .collect(),
        unprompted_states: Vec::new(),
        triggers: Vec::new(),
        closed_tasks: Vec::new(),
        chat: Vec::new(),
        message: input.into(),
    };
    let mut minimum = context.clone();
    for task in &mut minimum.open_tasks {
        if task.title.chars().count() > 1 {
            task.title = "…".into();
        }
    }
    if total(&instructions, &encode(&minimum)?, tuning) > limit {
        return Err(PromptError::RequiredContextTooLong);
    }
    fill_extras(
        &instructions,
        &mut context,
        &mut minimum,
        extras,
        tuning,
        limit,
    )?;
    context.closed_tasks = tasks
        .into_iter()
        .rev()
        .filter(|task| task.status != TaskStatus::Open)
        .map(PromptTask::from)
        .collect();
    let history = chat_history(day, input, now);
    let original = context
        .open_tasks
        .iter()
        .map(|task| task.title.clone())
        .collect::<Vec<_>>();
    let closed_count = context.closed_tasks.len();
    fit_tasks(&instructions, &mut context, &original, tuning, limit)?;
    // Keep a newest-first prefix. All older rows are necessarily dropped once
    // one row fails to fit, avoiding repeated encoding of an unbounded history.
    for message in &history {
        context.chat.push(HistoryRow {
            author: message.author,
            text: message.text.clone(),
        });
        if total(&instructions, &encode(&context)?, tuning) > limit {
            context.chat.pop();
            break;
        }
    }
    let prompt = encode(&context)?;
    let budget = BudgetReport {
        estimated_tokens: total(&instructions, &prompt, tuning),
        limit,
        open_tasks: context.open_tasks.len(),
        dropped_chat: history.len() - context.chat.len(),
        dropped_closed_tasks: closed_count - context.closed_tasks.len(),
        shortened_titles: context
            .open_tasks
            .iter()
            .zip(&original)
            .filter(|(task, title)| &task.title != *title)
            .count(),
    };
    Ok(BuiltChat {
        request: ModelRequest::new(&instructions, &prompt, CHAT_SCHEMA, tuning),
        budget,
    })
}
fn chat_history<'a>(day: &'a Day, input: &str, now: Now) -> Vec<&'a crate::day::Message> {
    let latest = day.messages().len().checked_sub(1);
    day.messages()
        .iter()
        .enumerate()
        .rev()
        .filter(|(index, message)| {
            !(Some(*index) == latest
                && message.author == Author::You
                && message.kind == MessageKind::Reply
                && message.time == now.instant
                && message.text == input)
        })
        .map(|(_, message)| message)
        .filter(|message| {
            matches!(message.author, Author::You | Author::Bunshin)
                && matches!(message.kind, MessageKind::Reply | MessageKind::Unprompted)
        })
        .collect::<Vec<_>>()
}
fn fit_tasks(
    instructions: &str,
    context: &mut Context,
    original: &[String],
    tuning: Tuning,
    limit: usize,
) -> Result<(), PromptError> {
    let mut title_limit = original
        .iter()
        .map(|title| title.chars().count())
        .max()
        .unwrap_or(1);
    while total(instructions, &encode(context)?, tuning) > limit {
        if context.closed_tasks.pop().is_some() {
            continue;
        }
        if title_limit <= 1 {
            return Err(PromptError::RequiredContextTooLong);
        }
        title_limit -= 1;
        for (task, title) in context.open_tasks.iter_mut().zip(original) {
            if title.chars().count() > title_limit {
                task.title = title.chars().take(title_limit - 1).chain(['…']).collect();
            }
        }
    }
    Ok(())
}
fn fill_extras(
    instructions: &str,
    context: &mut Context,
    minimum: &mut Context,
    extras: ContextExtras<'_>,
    tuning: Tuning,
    limit: usize,
) -> Result<(), PromptError> {
    if let Some(yesterday) = extras.yesterday {
        let mut text = String::new();
        for character in yesterday.chars() {
            text.push(character);
            if estimate(&text, tuning.prompt.ascii_chars_per_token) > tuning.prompt.yesterday_tokens
            {
                text.pop();
                break;
            }
        }
        minimum.yesterday = Some(text.clone());
        if total(instructions, &encode(minimum)?, tuning) <= limit {
            context.yesterday = Some(text);
        } else {
            minimum.yesterday = None;
        }
    }
    minimum.unprompted_states = extras
        .unprompted_states
        .iter()
        .rev()
        .take(tuning.prompt.recent_unprompted)
        .cloned()
        .collect();
    if total(instructions, &encode(minimum)?, tuning) <= limit {
        context
            .unprompted_states
            .clone_from(&minimum.unprompted_states);
    } else {
        minimum.unprompted_states.clear();
    }
    minimum.triggers = extras.triggers.to_vec();
    if total(instructions, &encode(minimum)?, tuning) <= limit {
        context.triggers.clone_from(&minimum.triggers);
    } else {
        minimum.triggers.clear();
    }
    Ok(())
}
