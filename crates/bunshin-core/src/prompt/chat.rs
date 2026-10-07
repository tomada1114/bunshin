//! A fresh owner-chat request with a conservative bound over the complete session.
use super::{budget::estimate, rules::operating_rules};
use crate::{
    ModelRequest, Now, Tuning,
    day::{Author, Day, Message, MessageKind, TaskKind, TaskStatus, TaskView},
};
use serde::Serialize;

const MODEL_WINDOW: usize = 4096;

/// Generation schema; domain validation separately rejects invalid individual changes.
pub const CHAT_SCHEMA: &str = r##"{"type":"object","properties":{"changes":{"type":"array","items":{"$ref":"#/$defs/Change"}},"reply":{"type":"string"}},"required":["changes","reply"],"additionalProperties":false,"title":"ChatAnswer","x-order":["reply","changes"],"$defs":{"Change":{"type":"object","properties":{"op":{"type":"string","enum":["add","done","drop","reopen","changeTime","rename"]},"task":{"type":"integer"},"title":{"type":"string"},"kind":{"type":"string","enum":["untimed","deadline","appointment"]},"time":{"type":"string"}},"required":["op"],"additionalProperties":false,"title":"Change","x-order":["op","task","title","kind","time"]}}}"##;

/// Numeric evidence for the request actually returned, without owner text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetReport {
    /// Rules + prompt + schema + answer reserve.
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
        /// Number of Unicode scalar values in the submitted input.
        chars: usize,
        /// Maximum permitted number of Unicode scalar values.
        limit: usize,
    },
    /// Even mandatory rules, current input and minimal task rows cannot fit.
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
    open_tasks: Vec<PromptTask>,
    closed_tasks: Vec<PromptTask>,
    chat: Vec<HistoryRow>,
    message: String,
}

fn encode(context: &Context) -> Result<String, PromptError> {
    serde_json::to_string(context).map_err(|_| PromptError::EncodingFailed)
}

fn total(rules: &str, prompt: &str, tuning: Tuning) -> usize {
    let ratio = tuning.prompt.ascii_chars_per_token;
    estimate(rules, ratio)
        .saturating_add(estimate(prompt, ratio))
        .saturating_add(estimate(CHAT_SCHEMA, ratio))
        .saturating_add(tuning.prompt.chat_answer_tokens)
}

/// Build one bounded request. Drop oldest chat, then closed tasks, then shorten open
/// titles in copies. Current input and every open identifier/time are mandatory.
/// # Errors
/// `InputTooLong` before assembly; `RequiredContextTooLong` if mandatory content cannot
/// fit even with minimal titles; `EncodingFailed` for context serialization failure.
pub fn build_chat(
    day: &Day,
    input: &str,
    now: Now,
    tuning: Tuning,
) -> Result<BuiltChat, PromptError> {
    let chars = input.chars().count();
    if chars > tuning.prompt.input_max_chars {
        return Err(PromptError::InputTooLong {
            chars,
            limit: tuning.prompt.input_max_chars,
        });
    }
    let rules = operating_rules(tuning);
    let limit = tuning.prompt.context_tokens.min(MODEL_WINDOW);
    let tasks = day.task_view();
    let mut context = Context {
        date: day.date().to_string(),
        now: now.local.to_string(),
        open_tasks: tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Open)
            .cloned()
            .map(PromptTask::from)
            .collect(),
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
    if total(&rules, &encode(&minimum)?, tuning) > limit {
        return Err(PromptError::RequiredContextTooLong);
    }
    let original = context
        .open_tasks
        .iter()
        .map(|task| task.title.clone())
        .collect::<Vec<_>>();
    let mut title_limit = original
        .iter()
        .map(|title| title.chars().count())
        .max()
        .unwrap_or(1);
    fit_open_tasks(&rules, &mut context, &original, title_limit, limit, tuning)?;
    context.closed_tasks = tasks
        .into_iter()
        .rev()
        .filter(|task| task.status != TaskStatus::Open)
        .map(PromptTask::from)
        .collect();
    let history = chat_history(day, input, now);
    let closed_count = context.closed_tasks.len();
    fit_to_limit(
        &rules,
        &mut context,
        &original,
        &mut title_limit,
        limit,
        tuning,
    )?;
    for message in &history {
        context.chat.push(HistoryRow {
            author: message.author,
            text: message.text.clone(),
        });
        if total(&rules, &encode(&context)?, tuning) > limit {
            context.chat.pop();
            break;
        }
    }
    fit_to_limit(
        &rules,
        &mut context,
        &original,
        &mut title_limit,
        limit,
        tuning,
    )?;
    let prompt = encode(&context)?;
    let budget = BudgetReport {
        estimated_tokens: total(&rules, &prompt, tuning),
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
        request: ModelRequest::new(&rules, &prompt, CHAT_SCHEMA, tuning),
        budget,
    })
}

fn chat_history<'a>(day: &'a Day, input: &str, now: Now) -> Vec<&'a Message> {
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
            !message.cancelled
                && matches!(message.author, Author::You | Author::Bunshin)
                && matches!(message.kind, MessageKind::Reply | MessageKind::Unprompted)
        })
        .collect()
}

fn fit_open_tasks(
    rules: &str,
    context: &mut Context,
    original: &[String],
    mut title_limit: usize,
    limit: usize,
    tuning: Tuning,
) -> Result<(), PromptError> {
    while total(rules, &encode(context)?, tuning) > limit {
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

fn fit_to_limit(
    rules: &str,
    context: &mut Context,
    original: &[String],
    title_limit: &mut usize,
    limit: usize,
    tuning: Tuning,
) -> Result<(), PromptError> {
    while total(rules, &encode(context)?, tuning) > limit {
        if context.closed_tasks.pop().is_some() {
            continue;
        }
        if *title_limit <= 1 {
            return Err(PromptError::RequiredContextTooLong);
        }
        *title_limit -= 1;
        for (task, title) in context.open_tasks.iter_mut().zip(original) {
            if title.chars().count() > *title_limit {
                task.title = title.chars().take(*title_limit - 1).chain(['…']).collect();
            }
        }
    }
    Ok(())
}
