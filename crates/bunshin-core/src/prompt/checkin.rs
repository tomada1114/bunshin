//! One check-in shares chat's bounded task context without proposing task changes.
use super::chat::{BuiltChat, ContextExtras, PromptError, RequestSpec, assemble};
use crate::{
    Now, Tuning,
    day::{Day, Trigger},
    instructions::InstructionsState,
};

/// Native fm schema names the object and generates message before optional numbers.
pub const CHECKIN_SCHEMA: &str = r#"{"type":"object","title":"CheckinAnswer","properties":{"kind":{"type":"string","enum":["silent","note","question"]},"message":{"type":"string"},"task":{"type":"integer"},"next_look_minutes":{"type":"integer"}},"required":["kind","message"],"additionalProperties":false,"x-order":["kind","message","task","next_look_minutes"]}"#;

/// Bounded check-in request and budget evidence, sharing the common request shape.
pub type BuiltCheckin = BuiltChat;

/// Build a single call for the whole ready batch. Every trigger is mandatory and
/// uses [code, task] tuples to retain the maximum fifty-task deadline batch.
/// # Errors
/// Refuses before calling the model if mandatory context cannot fit its window.
pub fn build_checkin(
    day: &Day,
    owner: &InstructionsState,
    now: Now,
    triggers: &[Trigger],
    extras: ContextExtras<'_>,
    tuning: Tuning,
) -> Result<BuiltCheckin, PromptError> {
    let rules = format!(
        "\n\n分身の見守り規則: 今日のタスクについて今声をかける必要があるか判断する。回答は kind, message, task, next_look_minutes の JSON。kind は silent（声をかけない）, note（短い助言）, question（返答を求める質問）。message は日本語で{}字以内を目安にする。task は存在する関連タスクの番号、一般的な話なら省略する。next_look_minutes は{}〜{}分、未指定なら{}分。タスクを変更しない。triggers は [種別, タスク番号または null] の配列: b=締切30分前, a=締切超過, p=予定の確認, s=日の開始, e=夕方の振り返り, c=再開。現在時刻と全タスクの事実を使い、同じ通知を繰り返さない。説明やコード囲みを JSON の外に書かない。",
        tuning.prompt.reply_max_chars,
        tuning.checkin.planned_min_minutes,
        tuning.checkin.planned_max_minutes,
        tuning.checkin.planned_default_minutes
    );
    assemble(
        day,
        owner,
        "",
        now,
        extras,
        tuning,
        &RequestSpec {
            schema: CHECKIN_SCHEMA,
            rules,
            answer_tokens: tuning.prompt.checkin_answer_tokens,
            ready_triggers: Some(triggers),
        },
    )
}
