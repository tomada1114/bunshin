//! The owner prompt carries only today's tasks and chat, under a fixed budget.
use bunshin_core::{
    Now, Tuning, UnixMillis,
    day::{Author, Day, Message, MessageKind, TaskKind, TaskOrigin, file::DayFile},
    prompt::{
        budget::estimate,
        chat::{PromptError, build_chat},
        rules::operating_rules,
    },
};
use jiff::civil::{date, time};

fn now() -> Now {
    Now {
        instant: UnixMillis(0),
        local: date(2026, 10, 2).at(14, 31, 0, 0),
    }
}

fn day() -> Day {
    Day::new(date(2026, 10, 2), Tuning::default())
}

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
    assert_eq!(estimate("abc", 0), 3);
}

#[test]
fn request_uses_only_built_in_rules_and_keeps_today_and_current_input() {
    let tuning = Tuning::default();
    let (day, _) = day()
        .add(
            "15時までに資料".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskOrigin::Chat,
            UnixMillis(1),
        )
        .expect("task");
    let built = build_chat(&day, "資料できた", now(), tuning).expect("request");

    assert_eq!(built.request.instructions, operating_rules(tuning));
    assert!(!built.request.instructions.contains("owner supplied"));
    assert!(built.request.instructions.contains("changes と reply"));
    let prompt: serde_json::Value = serde_json::from_str(&built.request.prompt).expect("JSON");
    assert_eq!(prompt["message"], "資料できた");
    assert_eq!(prompt["now"], "2026-10-02T14:31:00");
    assert_eq!(prompt["date"], "2026-10-02");
    assert_eq!(prompt["openTasks"][0]["number"], 1);
    assert_eq!(prompt["openTasks"][0]["title"], "15時までに資料");
    assert!(prompt.get("unpromptedStates").is_none());
    assert!(prompt.get("triggers").is_none());
    assert!(built.budget.estimated_tokens <= built.budget.limit);
}

#[test]
fn all_fifty_open_tasks_survive_budgeting_and_old_chat_is_dropped() {
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
            change_set: None,
            cancelled: false,
            in_reply_to: None,
        });
    }
    let day = file.into_day(tuning).expect("day");
    let built = build_chat(&day, "今日の状況を教えて", now(), tuning).expect("request");

    assert!(built.budget.estimated_tokens <= 4096);
    assert_eq!(built.budget.open_tasks, 50);
    assert_eq!(built.budget.dropped_chat, 300);
    assert!(built.budget.shortened_titles > 0);
    let prompt: serde_json::Value = serde_json::from_str(&built.request.prompt).expect("JSON");
    let rows = prompt["openTasks"].as_array().expect("open tasks");
    assert_eq!(rows.len(), 50);
    assert_eq!(
        rows.iter()
            .map(|row| row["number"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        (1..=50).collect::<Vec<_>>()
    );
    assert_eq!(
        day.tasks()[49].title.chars().count(),
        80,
        "source title stays intact"
    );
}

#[test]
fn input_limit_is_checked_before_prompt_assembly() {
    let tuning = Tuning::default();
    assert!(build_chat(&day(), &"文".repeat(400), now(), tuning).is_ok());
    assert_eq!(
        build_chat(&day(), &"文".repeat(401), now(), tuning),
        Err(PromptError::InputTooLong {
            chars: 401,
            limit: 400
        }),
    );
    let mut small = tuning;
    small.prompt.context_tokens = 1;
    assert_eq!(
        build_chat(&day(), "message", now(), small),
        Err(PromptError::RequiredContextTooLong)
    );
}

#[test]
fn a_current_message_already_saved_is_not_duplicated_in_chat_history() {
    let tuning = Tuning::default();
    let mut file = DayFile::from(&day());
    for instant in [-1, 0] {
        file.data.messages.push(Message {
            author: Author::You,
            text: "current".into(),
            time: UnixMillis(instant),
            kind: MessageKind::Reply,
            change_set: None,
            cancelled: false,
            in_reply_to: None,
        });
    }
    let day = file.into_day(tuning).expect("day");
    let built = build_chat(&day, "current", now(), tuning).expect("request");
    let prompt: serde_json::Value = serde_json::from_str(&built.request.prompt).expect("JSON");
    assert_eq!(prompt["message"], "current");
    assert_eq!(prompt["chat"].as_array().expect("chat").len(), 1);
}
