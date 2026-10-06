//! A fresh owner-chat request with a conservative bound over the complete session.
use super::{budget::estimate, rules::operating_rules};
use crate::{
    ModelRequest, Now, Tuning,
    day::{
        Author, Day, InboxState, MessageKind, TaskKind, TaskStatus, TaskView, Trigger, TriggerKind,
        UnpromptedKind,
    },
    instructions::InstructionsState,
};
use serde::Serialize;

const MODEL_WINDOW: usize = 4096;
// Only while leftovers are offered: their numbers belong to their own day.
const LEFTOVER_RULES: &str = "leftovers は前日の残り（番号は前日のもの）。持ち越しは carryOver、やめるは dropLeftover で task に leftovers の番号を入れる。持ち越したものは今日の時刻なしタスクになる。";
/// Generation schema; domain validation separately rejects invalid individual changes.
/// `fm` object names and field order follow its generated schema format. Reply
/// generation comes first; changes-first repeatedly stalled the empty-change
/// synthetic request on the local model (observed 2026-10-03).
pub const CHAT_SCHEMA: &str = r##"{"type":"object","properties":{"changes":{"type":"array","items":{"$ref":"#/$defs/Change"}},"reply":{"type":"string"}},"required":["changes","reply"],"additionalProperties":false,"title":"ChatAnswer","x-order":["reply","changes"],"$defs":{"Change":{"type":"object","properties":{"op":{"type":"string","enum":["add","done","drop","reopen","changeTime","rename","mute","carryOver","dropLeftover"]},"task":{"type":"integer"},"title":{"type":"string"},"kind":{"type":"string","enum":["untimed","deadline","appointment"]},"time":{"type":"string"},"minutes":{"type":"integer"}},"required":["op"],"additionalProperties":false,"title":"Change","x-order":["op","task","title","kind","time","minutes"]}}}"##;

/// Reaction facts passed to the model rather than private message text.
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
    /// Deterministic previous-day record, capped by its estimated token bound. When
    /// absent, the day's own stored record is used, so every call of the day has it.
    pub yesterday: Option<&'a str>,
    /// Undecided leftovers of the last day on record, numbered on their own day.
    /// Included in order while they fit, after today's open tasks.
    pub leftovers: &'a [TaskView],
    /// Fallback states in chronological order when the day has no delivered check-ins.
    /// Persisted inbox reactions take precedence; newest states are bounded by tuning.
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
#[serde(untagged)]
enum PromptTrigger {
    Detail(Trigger),
    Compact((&'static str, Option<u64>)),
}
impl PromptTrigger {
    fn compact(trigger: &Trigger) -> Self {
        let code = match trigger.kind {
            TriggerKind::BeforeDeadline => "b",
            TriggerKind::AfterDeadline => "a",
            TriggerKind::PlannedLook => "p",
            TriggerKind::DayStart => "s",
            TriggerKind::EveningReview => "e",
            TriggerKind::CatchUp => "c",
        };
        Self::Compact((code, trigger.task))
    }
}
pub(crate) struct RequestSpec<'a> {
    pub schema: &'static str,
    pub rules: String,
    pub answer_tokens: usize,
    pub ready_triggers: Option<&'a [Trigger]>,
}
#[derive(Clone, Copy)]
struct RequestBudget<'a> {
    instructions: &'a str,
    schema: &'static str,
    answer_tokens: usize,
    tuning: &'a Tuning,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Context {
    date: String,
    now: String,
    yesterday: Option<String>,
    open_tasks: Vec<PromptTask>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    leftovers: Vec<PromptTask>,
    unprompted_states: Vec<UnpromptedContext>,
    triggers: Vec<PromptTrigger>,
    closed_tasks: Vec<PromptTask>,
    chat: Vec<HistoryRow>,
    message: String,
}
fn encode(context: &Context) -> Result<String, PromptError> {
    serde_json::to_string(context).map_err(|_| PromptError::EncodingFailed)
}
fn total(budget: RequestBudget<'_>, prompt: &str) -> usize {
    let ratio = budget.tuning.prompt.ascii_chars_per_token;
    estimate(budget.instructions, ratio)
        .saturating_add(estimate(prompt, ratio))
        .saturating_add(estimate(budget.schema, ratio))
        .saturating_add(budget.answer_tokens)
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
    let mut rules = operating_rules(tuning);
    if !extras.leftovers.is_empty() {
        rules.push_str(LEFTOVER_RULES);
    }
    assemble(
        day,
        owner,
        input,
        now,
        extras,
        tuning,
        &RequestSpec {
            schema: CHAT_SCHEMA,
            rules,
            answer_tokens: tuning.prompt.chat_answer_tokens,
            ready_triggers: None,
        },
    )
}
pub(crate) fn assemble(
    day: &Day,
    owner: &InstructionsState,
    input: &str,
    now: Now,
    extras: ContextExtras<'_>,
    tuning: Tuning,
    spec: &RequestSpec<'_>,
) -> Result<BuiltChat, PromptError> {
    let chars = input.chars().count();
    if chars > tuning.prompt.input_max_chars {
        return Err(PromptError::InputTooLong {
            chars,
            limit: tuning.prompt.input_max_chars,
        });
    }
    let instructions = format!("{}{}", owner.text, spec.rules);
    let budget = RequestBudget {
        instructions: &instructions,
        schema: spec.schema,
        answer_tokens: spec.answer_tokens,
        tuning: &tuning,
    };
    let limit = tuning.prompt.context_tokens.min(MODEL_WINDOW);
    let tasks = day.task_view();
    let mut context = Context {
        date: day.date().to_string(),
        now: now.local.to_string(),
        yesterday: None,
        leftovers: Vec::new(),
        open_tasks: tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Open)
            .cloned()
            .map(PromptTask::from)
            .collect(),
        unprompted_states: Vec::new(),
        triggers: spec.ready_triggers.map_or_else(Vec::new, |triggers| {
            triggers.iter().map(PromptTrigger::compact).collect()
        }),
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
    if total(budget, &encode(&minimum)?) > limit {
        return Err(PromptError::RequiredContextTooLong);
    }
    let original = context
        .open_tasks
        .iter()
        .map(|task| task.title.clone())
        .collect::<Vec<_>>();
    fill_yesterday(budget, &mut context, &mut minimum, extras.yesterday, day)?;
    // Open names precede leftovers, recent states and triggers. Establish their
    // title lengths before admitting any lower-priority optional block.
    fit_tasks(budget, &mut context, &original, limit)?;
    fill_leftovers(budget, &mut context, extras.leftovers, limit)?;
    fill_lower_extras(
        budget,
        &mut context,
        extras,
        limit,
        spec.ready_triggers.is_none(),
        day,
        now,
    )?;
    context.closed_tasks = tasks
        .into_iter()
        .rev()
        .filter(|task| task.status != TaskStatus::Open)
        .map(PromptTask::from)
        .collect();
    let history = chat_history(day, input, now);
    let closed_count = context.closed_tasks.len();
    fit_tasks(budget, &mut context, &original, limit)?;
    // Keep a newest-first prefix. All older rows are necessarily dropped once
    // one row fails to fit, avoiding repeated encoding of an unbounded history.
    for message in &history {
        context.chat.push(HistoryRow {
            author: message.author,
            text: message.text.clone(),
        });
        if total(budget, &encode(&context)?) > limit {
            context.chat.pop();
            break;
        }
    }
    let prompt = encode(&context)?;
    let budget = BudgetReport {
        estimated_tokens: total(budget, &prompt),
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
        request: ModelRequest::new(&instructions, &prompt, spec.schema, tuning),
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
            message
                .unprompted
                .as_ref()
                .is_none_or(|extra| extra.suppressed.is_none())
        })
        .filter(|message| {
            !message.cancelled
                && matches!(message.author, Author::You | Author::Bunshin)
                && matches!(message.kind, MessageKind::Reply | MessageKind::Unprompted)
        })
        .collect::<Vec<_>>()
}
fn fit_tasks(
    budget: RequestBudget<'_>,
    context: &mut Context,
    original: &[String],
    limit: usize,
) -> Result<(), PromptError> {
    let mut title_limit = original
        .iter()
        .map(|title| title.chars().count())
        .max()
        .unwrap_or(1);
    while total(budget, &encode(context)?) > limit {
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
// A caller's record wins; otherwise the day's stored record keeps every call carrying it.
fn fill_yesterday(
    budget: RequestBudget<'_>,
    context: &mut Context,
    minimum: &mut Context,
    yesterday: Option<&str>,
    day: &Day,
) -> Result<(), PromptError> {
    let limit = budget.tuning.prompt.context_tokens.min(MODEL_WINDOW);
    let stored = day.data().yesterday_record.as_ref();
    if let Some(yesterday) = yesterday.or(stored.map(|record| record.text.as_str())) {
        let mut text = String::new();
        for character in yesterday.chars() {
            text.push(character);
            if estimate(&text, budget.tuning.prompt.ascii_chars_per_token)
                > budget.tuning.prompt.yesterday_tokens
            {
                text.pop();
                break;
            }
        }
        minimum.yesterday = Some(text.clone());
        if total(budget, &encode(minimum)?) <= limit {
            context.yesterday = Some(text);
        } else {
            minimum.yesterday = None;
        }
    }
    Ok(())
}
// Leftovers follow today's open tasks, each only while it still fits.
fn fill_leftovers(
    budget: RequestBudget<'_>,
    context: &mut Context,
    leftovers: &[TaskView],
    limit: usize,
) -> Result<(), PromptError> {
    for leftover in leftovers {
        context.leftovers.push(PromptTask::from(leftover.clone()));
        if total(budget, &encode(context)?) > limit {
            context.leftovers.pop();
            break;
        }
    }
    Ok(())
}
fn fill_lower_extras(
    budget: RequestBudget<'_>,
    context: &mut Context,
    extras: ContextExtras<'_>,
    limit: usize,
    optional_triggers: bool,
    day: &Day,
    now: Now,
) -> Result<(), PromptError> {
    let states = day.inbox_context(now.instant);
    let extras = ContextExtras {
        unprompted_states: if states.is_empty() {
            extras.unprompted_states
        } else {
            &states
        },
        ..extras
    };
    let mut candidate = context.clone();
    candidate.unprompted_states = extras
        .unprompted_states
        .iter()
        .rev()
        .take(budget.tuning.prompt.recent_unprompted)
        .cloned()
        .collect();
    if total(budget, &encode(&candidate)?) <= limit {
        context
            .unprompted_states
            .clone_from(&candidate.unprompted_states);
    } else {
        candidate.unprompted_states.clear();
    }
    if optional_triggers {
        candidate.triggers = extras
            .triggers
            .iter()
            .cloned()
            .map(PromptTrigger::Detail)
            .collect();
        if total(budget, &encode(&candidate)?) <= limit {
            context.triggers.clone_from(&candidate.triggers);
        }
    }
    Ok(())
}
