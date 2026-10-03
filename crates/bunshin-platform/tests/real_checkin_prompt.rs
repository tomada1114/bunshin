//! Local compatibility of one synthetic check-in schema with the actual fm command.
#![cfg(target_os = "macos")]
use bunshin_core::{
    CancelFlag, LanguageModel, Now, Tuning, UnixMillis,
    checkin::answer::parse_checkin,
    day::{Day, TaskKind, TaskOrigin, Trigger, TriggerKind},
    instructions::InstructionsState,
    prompt::{chat::ContextExtras, checkin::build_checkin},
};
use bunshin_platform::FmLanguageModel;

#[test]
#[ignore = "local machine: fm with Apple Intelligence enabled"]
fn actual_checkin_schema_and_bounded_request_produce_a_valid_non_mutating_answer() {
    let tuning = Tuning::default();
    let now = Now {
        instant: UnixMillis(0),
        local: jiff::civil::date(2026, 10, 3).at(14, 30, 0, 0),
    };
    let day = Day::new(now.local.date(), tuning)
        .add(
            "合成タスク".into(),
            TaskKind::Deadline,
            Some(jiff::civil::time(15, 0, 0, 0)),
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap()
        .0;
    let tasks = day.tasks().to_vec();
    let owner = InstructionsState::resolve(None, "instructions.md".into(), tuning);
    let triggers = [Trigger {
        kind: TriggerKind::BeforeDeadline,
        task: Some(1),
        due_at: now.instant,
    }];
    let built = build_checkin(
        &day,
        &owner,
        now,
        &triggers,
        ContextExtras::default(),
        tuning,
    )
    .unwrap();
    assert!(built.budget.estimated_tokens <= 4096);
    let answer = FmLanguageModel::default()
        .respond(&built.request, &CancelFlag::default())
        .expect("actual fm accepts the native check-in schema");
    let parsed = parse_checkin(&answer.json, &day, tuning)
        .expect("actual answer passes strict check-in validation");
    assert!((5..=120).contains(&parsed.next_look_minutes));
    assert_eq!(day.tasks(), tasks);
    assert!(!built.request.schema.contains("changes"));
}
