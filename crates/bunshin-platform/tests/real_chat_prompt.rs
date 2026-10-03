//! Local compatibility of core's full chat schema with the macOS model command.
#![cfg(target_os = "macos")]
use bunshin_core::{
    CancelFlag, LanguageModel, Now, Tuning, UnixMillis,
    day::Day,
    instructions::InstructionsState,
    prompt::{
        answer::apply_chat,
        chat::{ContextExtras, build_chat},
    },
};
use bunshin_platform::FmLanguageModel;
use jiff::civil::date;

#[test]
#[ignore = "local machine: fm with Apple Intelligence enabled"]
fn actual_chat_schema_and_request_are_accepted_and_answer_can_be_validated() {
    let tuning = Tuning::default();
    let day = Day::new(date(2026, 10, 2), tuning);
    let owner = InstructionsState::resolve(None, "instructions.md".into(), tuning);
    let now = Now {
        instant: UnixMillis(0),
        local: date(2026, 10, 2).at(14, 0, 0, 0),
    };
    let built = build_chat(
        &day,
        &owner,
        "タスクの追加・変更は不要です。changes は []、reply は「了解」としてください。",
        now,
        ContextExtras::default(),
        tuning,
    )
    .expect("bounded request");
    assert!(built.budget.estimated_tokens <= 4096);
    let answer = FmLanguageModel::default()
        .respond(&built.request, &CancelFlag::default())
        .expect("real fm accepts the schema");
    let outcome = apply_chat(&day, &answer, now.instant, tuning)
        .expect("real fm answer matches the chat schema");
    assert!(
        outcome.change_set.is_none(),
        "the synthetic request asks for no task changes"
    );
    assert!(!outcome.reply.is_empty());

    // These requests belong to one serial local compatibility scenario.
    let tuning = Tuning::default();
    let day = Day::new(date(2026, 10, 2), tuning);
    let owner = InstructionsState::resolve(None, "instructions.md".into(), tuning);
    let now = Now {
        instant: UnixMillis(0),
        local: date(2026, 10, 2).at(14, 0, 0, 0),
    };
    let built = build_chat(&day, &owner, "15時までの合成タスクを追加して。changes は op=add,title=合成タスク,kind=deadline,time=15:00 の1件。reply は了解。", now, ContextExtras::default(), tuning).expect("bounded request");
    let answer = FmLanguageModel::default()
        .respond(&built.request, &CancelFlag::default())
        .expect("real fm accepts the nested proposal schema");
    let outcome =
        apply_chat(&day, &answer, now.instant, tuning).expect("real proposal matches the schema");
    assert!(outcome.refused.is_empty());
    assert_eq!(outcome.day.tasks().len(), 1);
    assert_eq!(
        outcome.day.tasks()[0].kind,
        bunshin_core::day::TaskKind::Deadline
    );
    assert_eq!(
        outcome.day.tasks()[0].time,
        Some(jiff::civil::time(15, 0, 0, 0))
    );
    assert_eq!(
        outcome.change_set.expect("one visible batch").changes.len(),
        1
    );
}
