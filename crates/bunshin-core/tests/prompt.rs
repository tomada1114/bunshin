//! Prompt budgets are deterministic estimates over scalar values, never model calls.
use bunshin_core::prompt::budget::estimate;
use bunshin_core::{
    Now, Tuning, UnixMillis,
    day::{Author, Day, Message, MessageKind, TaskKind, TaskOrigin, file::DayFile},
    instructions::InstructionsState,
    prompt::chat::{ContextExtras, PromptError, build_chat},
};

#[test]
fn budget_estimate_rounds_ascii_up_and_counts_each_non_ascii_scalar() {
    for (text, expected) in [
        ("", 0),
        ("a", 1),
        ("ab", 1),
        ("abc", 2),
        ("abcd", 2),
        ("日本語", 3),
        ("a日本bc", 4),
        ("e\u{301}", 2),
        ("👨‍👩‍👧‍👦", 7),
    ] {
        assert_eq!(estimate(text, 2), expected, "{text:?}");
    }
    assert_eq!(estimate("abc日本", 1), 5);
    assert_eq!(estimate("abc日本", 3), 3);
    assert_eq!(
        estimate("abc", 0),
        3,
        "invalid zero uses the conservative one-char estimate"
    );
}

fn now() -> Now {
    Now {
        instant: UnixMillis(0),
        local: jiff::civil::date(2026, 10, 2).at(14, 31, 0, 0),
    }
}
fn instructions(text: &str) -> InstructionsState {
    InstructionsState::resolve(Some(text), "instructions.md".into(), Tuning::default())
}
fn day() -> Day {
    Day::new(jiff::civil::date(2026, 10, 2), Tuning::default())
}

#[test]
fn request_budget_retains_all_fifty_open_tasks_with_six_hundred_instructions_chars() {
    let tuning = Tuning::default();
    let mut day = day();
    for n in 1..=50 {
        day = day
            .add(
                format!("{n}{}", "題".repeat(78)),
                TaskKind::Untimed,
                None,
                TaskOrigin::Chat,
                UnixMillis(0),
            )
            .expect("task")
            .0;
    }
    let mut file = DayFile::from(&day);
    for index in 0..300 {
        file.data.messages.push(Message {
            author: Author::You,
            text: format!("old chat {index}{}", "話".repeat(400)),
            time: UnixMillis(index),
            kind: MessageKind::Reply,
            unprompted: None,
            answers_question: None,
            change_set: None,
        });
    }
    let day = file.into_day(tuning).expect("day");
    let built = build_chat(
        &day,
        &instructions(&"指".repeat(600)),
        "今日の状況を教えて",
        now(),
        ContextExtras::default(),
        tuning,
    )
    .expect("request");
    let cost = estimate(&built.request.instructions, 2)
        + estimate(&built.request.prompt, 2)
        + estimate(&built.request.schema, 2)
        + 450;
    assert!(cost <= 4096, "{cost}");
    assert_eq!(built.budget.estimated_tokens, cost);
    assert_eq!(built.budget.open_tasks, 50);
    assert_eq!(built.budget.dropped_chat, 300);
    assert!(built.budget.shortened_titles > 0);
    let prompt: serde_json::Value =
        serde_json::from_str(&built.request.prompt).expect("context JSON");
    assert_eq!(
        prompt["openTasks"].as_array().expect("open tasks").len(),
        50
    );
    let numbers = prompt["openTasks"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["number"].as_u64().expect("number"))
        .collect::<Vec<_>>();
    assert_eq!(numbers, (1..=50).collect::<Vec<_>>());
    assert_eq!(
        day.tasks()[49].title.chars().count(),
        80,
        "source title untouched"
    );
}

#[test]
fn request_appends_operating_rules_after_owner_text_and_keeps_the_current_message() {
    let tuning = Tuning::default();
    let built = build_chat(
        &day(),
        &instructions("owner text"),
        "15時までに資料",
        now(),
        ContextExtras::default(),
        tuning,
    )
    .expect("request");
    assert!(built.request.instructions.starts_with("owner text\n\n"));
    assert!(built.request.instructions.contains("200字"));
    assert!(built.request.instructions.contains("明示された時刻"));
    let prompt: serde_json::Value = serde_json::from_str(&built.request.prompt).expect("JSON");
    assert_eq!(prompt["message"], "15時までに資料");
    assert_eq!(prompt["now"], "2026-10-02T14:31:00");
    assert_eq!(prompt["date"], "2026-10-02");
    assert_eq!(prompt["openTasks"], serde_json::json!([]));
    assert_eq!(prompt["unpromptedStates"], serde_json::json!([]));
    assert_eq!(prompt["triggers"], serde_json::json!([]));
    assert!(built.budget.estimated_tokens <= 4096);
}

#[test]
fn four_hundred_scalar_input_is_allowed_and_four_hundred_one_is_refused() {
    let tuning = Tuning::default();
    let owner = instructions("owner");
    assert!(
        build_chat(
            &day(),
            &owner,
            &"文".repeat(400),
            now(),
            ContextExtras::default(),
            tuning
        )
        .is_ok()
    );
    assert_eq!(
        build_chat(
            &day(),
            &owner,
            &"文".repeat(401),
            now(),
            ContextExtras::default(),
            tuning
        ),
        Err(PromptError::InputTooLong {
            chars: 401,
            limit: 400
        })
    );
    let mut small = tuning;
    small.prompt.context_tokens = 1;
    assert!(matches!(
        build_chat(
            &day(),
            &owner,
            "message",
            now(),
            ContextExtras::default(),
            small
        ),
        Err(PromptError::RequiredContextTooLong)
    ));
}
#[test]
fn optional_context_is_bounded_and_newest_chat_is_kept_before_oldest() {
    use bunshin_core::day::{InboxState, Trigger, TriggerKind, UnpromptedKind};
    use bunshin_core::prompt::chat::UnpromptedContext;
    let tuning = Tuning::default();
    let mut file = DayFile::from(&day());
    for index in 0..30 {
        file.data.messages.push(Message {
            author: Author::You,
            text: format!("chat{index}:{}", "話".repeat(400)),
            time: UnixMillis(index),
            kind: MessageKind::Reply,
            unprompted: None,
            answers_question: None,
            change_set: None,
        });
    }
    file.data.messages.push(Message {
        author: Author::System,
        text: "system detail must stay out".into(),
        time: UnixMillis(31),
        kind: MessageKind::Error,
        unprompted: None,
        answers_question: None,
        change_set: None,
    });
    let day = file.into_day(tuning).expect("valid test fixture");
    let states = (0..10)
        .map(|index| UnpromptedContext {
            task: Some(index),
            kind: UnpromptedKind::Question,
            state: InboxState::Ignored,
        })
        .collect::<Vec<_>>();
    let triggers = [Trigger {
        kind: TriggerKind::BeforeDeadline,
        task: Some(1),
        due_at: UnixMillis(0),
    }];
    let built = build_chat(
        &day,
        &instructions("owner"),
        "current",
        now(),
        ContextExtras {
            yesterday: Some(&"昨".repeat(200)),
            unprompted_states: &states,
            triggers: &triggers,
        },
        tuning,
    )
    .expect("valid test fixture");
    let json: serde_json::Value =
        serde_json::from_str(&built.request.prompt).expect("valid test fixture");
    assert_eq!(
        estimate(json["yesterday"].as_str().expect("valid test fixture"), 2),
        120
    );
    assert_eq!(
        json["unpromptedStates"]
            .as_array()
            .expect("valid test fixture")
            .len(),
        5
    );
    assert_eq!(json["unpromptedStates"][0]["task"], 9);
    assert_eq!(json["unpromptedStates"][4]["task"], 5);
    assert_eq!(json["triggers"][0]["kind"], "beforeDeadline");
    let chat = json["chat"].as_array().expect("valid test fixture");
    assert!(!chat.is_empty());
    assert!(chat.len() < 30);
    assert!(
        chat[0]["text"]
            .as_str()
            .expect("valid test fixture")
            .starts_with("chat29:")
    );
    assert_eq!(built.budget.dropped_chat, 30 - chat.len());
    assert!(!built.request.prompt.contains("system detail"));
    assert!(built.budget.estimated_tokens <= 4096);
}
#[test]
fn history_is_dropped_before_closed_tasks_and_closed_tasks_before_open_title_shortening() {
    let tuning = Tuning::default();
    let mut day = day();
    for n in 1..=5 {
        day = day
            .add(
                format!("{n}{}", "題".repeat(78)),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                UnixMillis(0),
            )
            .expect("valid test fixture")
            .0;
    }
    day = day.done(5, UnixMillis(1)).expect("valid test fixture").0;
    let owner = instructions("owner");
    let base = build_chat(
        &day,
        &owner,
        "current",
        now(),
        ContextExtras::default(),
        tuning,
    )
    .expect("valid test fixture");
    let mut file = DayFile::from(&day);
    file.data.messages.push(Message {
        author: Author::You,
        text: "話".repeat(400),
        time: UnixMillis(2),
        kind: MessageKind::Reply,
        unprompted: None,
        answers_question: None,
        change_set: None,
    });
    let day = file.into_day(tuning).expect("valid test fixture");
    let mut small = tuning;
    small.prompt.context_tokens = base.budget.estimated_tokens;
    let built = build_chat(
        &day,
        &owner,
        "current",
        now(),
        ContextExtras::default(),
        small,
    )
    .expect("valid test fixture");
    assert_eq!(built.budget.dropped_chat, 1);
    assert_eq!(built.budget.dropped_closed_tasks, 0);
    assert_eq!(built.budget.shortened_titles, 0);
    small.prompt.context_tokens -= 1;
    let built = build_chat(
        &day,
        &owner,
        "current",
        now(),
        ContextExtras::default(),
        small,
    )
    .expect("valid test fixture");
    assert_eq!(built.budget.dropped_chat, 1);
    assert_eq!(built.budget.dropped_closed_tasks, 1);
    assert_eq!(built.budget.shortened_titles, 0);
    let json: serde_json::Value =
        serde_json::from_str(&built.request.prompt).expect("valid test fixture");
    assert_eq!(
        json["openTasks"]
            .as_array()
            .expect("valid test fixture")
            .len(),
        4
    );
}
#[test]
fn tiny_optional_slots_never_displace_mandatory_context_and_tuning_cannot_raise_the_window() {
    use bunshin_core::day::{InboxState, Trigger, TriggerKind, UnpromptedKind};
    use bunshin_core::prompt::chat::UnpromptedContext;
    let tuning = Tuning::default();
    let owner = instructions("owner");
    let base = build_chat(
        &day(),
        &owner,
        "current",
        now(),
        ContextExtras::default(),
        tuning,
    )
    .expect("valid test fixture");
    let mut small = tuning;
    small.prompt.context_tokens = base.budget.estimated_tokens;
    let states = [UnpromptedContext {
        task: Some(1),
        kind: UnpromptedKind::Note,
        state: InboxState::Open,
    }];
    let triggers = [Trigger {
        kind: TriggerKind::PlannedLook,
        task: None,
        due_at: UnixMillis(0),
    }];
    let built = build_chat(
        &day(),
        &owner,
        "current",
        now(),
        ContextExtras {
            yesterday: Some(&"昨".repeat(200)),
            unprompted_states: &states,
            triggers: &triggers,
        },
        small,
    )
    .expect("valid test fixture");
    let json: serde_json::Value =
        serde_json::from_str(&built.request.prompt).expect("valid test fixture");
    assert!(json["yesterday"].is_null());
    assert_eq!(json["unpromptedStates"], serde_json::json!([]));
    assert_eq!(json["triggers"], serde_json::json!([]));
    assert_eq!(json["message"], "current");
    assert!(built.budget.estimated_tokens <= small.prompt.context_tokens);
    let mut larger = tuning;
    larger.prompt.context_tokens = usize::MAX;
    assert_eq!(
        build_chat(
            &day(),
            &owner,
            "current",
            now(),
            ContextExtras::default(),
            larger
        )
        .expect("valid test fixture")
        .budget
        .limit,
        4096
    );
}
#[test]
fn budget_refusal_codes_and_request_debug_exclude_private_text() {
    let tuning = Tuning::default();
    let built = build_chat(
        &day(),
        &instructions("private owner text"),
        "private message",
        now(),
        ContextExtras::default(),
        tuning,
    )
    .expect("valid test fixture");
    let debug = format!("{built:?}");
    assert!(!debug.contains("private owner"));
    assert!(!debug.contains("private message"));
    assert_eq!(
        serde_json::to_value(PromptError::InputTooLong {
            chars: 401,
            limit: 400
        })
        .expect("valid test fixture"),
        serde_json::json!({"code":"inputTooLong","chars":401,"limit":400})
    );
    assert_eq!(
        serde_json::to_value(PromptError::RequiredContextTooLong).expect("valid test fixture"),
        serde_json::json!({"code":"requiredContextTooLong"})
    );
    assert_eq!(
        serde_json::to_value(PromptError::EncodingFailed).expect("valid test fixture"),
        serde_json::json!({"code":"encodingFailed"})
    );
}
#[test]
fn already_appended_current_message_is_present_once_but_previous_equal_messages_remain() {
    let tuning = Tuning::default();
    let mut file = DayFile::from(&day());
    for instant in [-1, 0] {
        file.data.messages.push(Message {
            author: Author::You,
            text: "current".into(),
            time: UnixMillis(instant),
            kind: MessageKind::Reply,
            unprompted: None,
            answers_question: None,
            change_set: None,
        });
    }
    let day = file.into_day(tuning).expect("valid test fixture");
    let built = build_chat(
        &day,
        &instructions("owner"),
        "current",
        now(),
        ContextExtras::default(),
        tuning,
    )
    .expect("valid test fixture");
    let json: serde_json::Value =
        serde_json::from_str(&built.request.prompt).expect("valid test fixture");
    assert_eq!(json["message"], "current");
    assert_eq!(
        json["chat"].as_array().expect("valid test fixture").len(),
        1
    );
}
#[test]
fn maximum_owner_input_and_fifty_maximum_titles_fit_together() {
    let tuning = Tuning::default();
    for (kind, time) in [
        (TaskKind::Untimed, None),
        (TaskKind::Deadline, Some(jiff::civil::time(15, 0, 0, 0))),
        (TaskKind::Appointment, Some(jiff::civil::time(15, 0, 0, 0))),
    ] {
        let mut day = day();
        for _ in 0..50 {
            day = day
                .add("題".repeat(80), kind, time, TaskOrigin::Chat, UnixMillis(0))
                .expect("task")
                .0;
        }
        let built = build_chat(
            &day,
            &instructions(&"指".repeat(600)),
            &"文".repeat(400),
            now(),
            ContextExtras::default(),
            tuning,
        )
        .expect("every supported maximum still fits");
        assert_eq!(built.budget.open_tasks, 50);
        assert!(built.budget.estimated_tokens <= 4096);
        let json: serde_json::Value =
            serde_json::from_str(&built.request.prompt).expect("prompt JSON");
        for task in json["openTasks"].as_array().expect("all open tasks") {
            assert_eq!(task["kind"], serde_json::to_value(kind).expect("kind"));
            assert_eq!(
                task["time"],
                time.map_or(serde_json::Value::Null, |_| serde_json::json!("15:00"))
            );
        }
    }
}
