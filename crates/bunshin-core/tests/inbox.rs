//! Inbox reactions use persisted rows and supplied instants, with no model dependency.
use bunshin_core::{
    Now, Tuning, UnixMillis,
    day::{
        Author, Day, InboxState, Message, MessageKind, Trigger, TriggerKind, UnpromptedKind,
        UnpromptedMessage, file::DayFile,
    },
    inbox::InboxAction,
    screen::{Effect, Focus, MainScreen, ScreenKey},
};
use jiff::civil::date;

fn fixture(kinds: &[UnpromptedKind]) -> Day {
    let tuning = Tuning::default();
    let mut data = Day::new(date(2026, 10, 4), tuning).data().clone();
    for (index, kind) in kinds.iter().enumerate() {
        let time = UnixMillis(i64::try_from(index).unwrap_or(0) * 60_000);
        data.messages.push(Message {
            author: Author::Bunshin,
            text: format!("synthetic {index}"),
            time,
            kind: MessageKind::Unprompted,
            answers_question: None,
            change_set: None,
            unprompted: Some(UnpromptedMessage {
                kind: *kind,
                trigger: Trigger {
                    kind: TriggerKind::PlannedLook,
                    task: None,
                    due_at: time,
                },
                task: None,
                inbox_state: InboxState::Open,
                state_changed_at: time,
                suppressed: None,
            }),
        });
    }
    match (DayFile { format: 1, data }).into_day(tuning) {
        Ok(day) => day,
        Err(error) => panic!("fixture refused: {error}"),
    }
}
fn now(at: i64) -> Now {
    Now {
        instant: UnixMillis(at),
        local: date(2026, 10, 4).at(14, 30, 0, 0),
    }
}

#[test]
fn inbox_lists_newest_open_rows_and_reports_ignored_without_closing_them() {
    let day = fixture(&[UnpromptedKind::Note, UnpromptedKind::Question]);
    let view = day.inbox_view();
    assert_eq!(
        view.items
            .iter()
            .map(|item| item.message)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(view.header_count(), Some(2));
    let states = day.inbox_context(UnixMillis(900_000));
    assert_eq!(
        states.iter().map(|item| item.state).collect::<Vec<_>>(),
        [InboxState::Ignored, InboxState::Open]
    );
    assert_eq!(day.inbox_view().items.len(), 2);
}

#[test]
fn implicit_reply_targets_the_newest_question_strictly_within_the_window() {
    let day = fixture(&[UnpromptedKind::Question, UnpromptedKind::Question]);
    assert_eq!(day.implicit_reply_target(UnixMillis(959_999)), Some(1));
    assert_eq!(day.implicit_reply_target(UnixMillis(960_000)), None);
    let replied = day.record_owner_message("synthetic reply", None, UnixMillis(120_000));
    assert_eq!(replied.messages().last().unwrap().answers_question, Some(1));
    assert_eq!(
        replied.messages()[1]
            .unprompted
            .as_ref()
            .unwrap()
            .inbox_state,
        InboxState::Answered
    );
    assert_eq!(replied.inbox_view().items.len(), 1);
}

#[test]
fn inbox_keys_acknowledge_notes_dismiss_questions_and_save_only_mutations() {
    let day = fixture(&[UnpromptedKind::Note, UnpromptedKind::Question]);
    let (screen, _) = MainScreen::new(day, Tuning::default()).update(ScreenKey::Tab, now(0));
    let screen = screen.open_inbox();
    assert_eq!(screen.inbox().unwrap().items.len(), 2);
    let (screen, effects) = screen.update(ScreenKey::Char('x'), now(120_000));
    assert_eq!(effects, [Effect::Save]);
    assert_eq!(
        screen.day().messages()[1]
            .unprompted
            .as_ref()
            .unwrap()
            .inbox_state,
        InboxState::Dismissed
    );
    let (screen, effects) = screen.update(ScreenKey::Enter, now(180_000));
    assert_eq!(effects, [Effect::Save]);
    assert_eq!(screen.inbox().unwrap().header_count(), None);
    let (screen, effects) = screen.update(ScreenKey::Char('X'), now(240_000));
    assert!(effects.is_empty());
    assert_eq!(
        screen.day().inbox_context(UnixMillis(240_000))[0].state,
        InboxState::Acknowledged
    );
}

#[test]
fn explicit_question_selection_survives_age_until_the_owner_sends() {
    let day = fixture(&[UnpromptedKind::Question]);
    let (screen, _) = MainScreen::new(day, Tuning::default()).update(ScreenKey::Tab, now(0));
    let screen = screen.open_inbox();
    let (screen, effects) = screen.update(ScreenKey::Enter, now(1_800_000));
    assert!(effects.is_empty());
    assert_eq!(screen.focus(), Focus::Input);
    assert_eq!(screen.reply_target(), Some(0));
    assert!(screen.inbox().is_none());
    let (screen, effects) = screen.submit_owner_message("synthetic reply", now(1_860_000));
    assert_eq!(effects, [Effect::Save]);
    assert_eq!(screen.reply_target(), None);
    assert_eq!(
        screen.day().messages().last().unwrap().answers_question,
        Some(0)
    );
}

#[test]
fn acknowledging_all_notes_leaves_questions_and_roundtrips_reaction_times() {
    let day = fixture(&[
        UnpromptedKind::Note,
        UnpromptedKind::Question,
        UnpromptedKind::Note,
    ]);
    let (day, changed) =
        day.react_to_inbox(None, InboxAction::AcknowledgeNotes, UnixMillis(180_000));
    assert!(changed);
    assert_eq!(
        day.inbox_view()
            .items
            .iter()
            .map(|item| item.message)
            .collect::<Vec<_>>(),
        [1]
    );
    let json = serde_json::to_string(&DayFile::from(&day)).unwrap();
    let loaded = serde_json::from_str::<DayFile>(&json)
        .unwrap()
        .into_day(Tuning::default())
        .unwrap();
    assert_eq!(loaded.data(), day.data());
    assert_eq!(
        loaded.messages()[0]
            .unprompted
            .as_ref()
            .unwrap()
            .state_changed_at,
        UnixMillis(180_000)
    );
}

#[test]
fn inbox_navigation_keeps_the_chosen_older_question_and_captures_other_keys() {
    let day = fixture(&[UnpromptedKind::Question, UnpromptedKind::Question]);
    let (screen, _) = MainScreen::new(day, Tuning::default()).update(ScreenKey::Tab, now(0));
    let (screen, effects) = screen.open_inbox().update(ScreenKey::Down, now(0));
    assert!(effects.is_empty());
    assert_eq!(screen.inbox().unwrap().selected, Some(1));
    let (screen, effects) = screen.update(ScreenKey::Char('a'), now(0));
    assert!(effects.is_empty());
    assert!(screen.form().is_none());
    let (screen, _) = screen.update(ScreenKey::Enter, now(0));
    assert_eq!(screen.reply_target(), Some(0));
}

#[test]
fn both_prompt_paths_take_the_last_five_reactions_from_the_day() {
    use bunshin_core::{
        instructions::InstructionsState,
        prompt::{
            chat::{ContextExtras, build_chat},
            checkin::build_checkin,
        },
    };
    let day = fixture(&[UnpromptedKind::Note; 6]);
    let mut data = day.data().clone();
    // Distinct reactions prove the oldest row, rather than any arbitrary note,
    // is excluded; the newest acknowledgement is kept by both prompt paths.
    data.messages[0].unprompted.as_mut().unwrap().inbox_state = InboxState::Muted;
    let day = (DayFile { format: 1, data })
        .into_day(Tuning::default())
        .unwrap();
    let owner = InstructionsState::resolve(
        Some("synthetic instructions"),
        "instructions.md".into(),
        Tuning::default(),
    );
    let (day, _) = day.react_to_inbox(Some(5), InboxAction::Close, UnixMillis(420_000));
    let chat = build_chat(
        &day,
        &owner,
        "synthetic message",
        now(420_000),
        ContextExtras::default(),
        Tuning::default(),
    )
    .unwrap();
    let checkin = build_checkin(
        &day,
        &owner,
        now(420_000),
        &[],
        ContextExtras::default(),
        Tuning::default(),
    )
    .unwrap();
    for built in [chat, checkin] {
        let context: serde_json::Value = serde_json::from_str(&built.request.prompt).unwrap();
        let states = context["unpromptedStates"].as_array().unwrap();
        assert_eq!(states.len(), 5);
        assert_eq!(
            states[0],
            serde_json::json!({"task":null,"kind":"note","state":"acknowledged"})
        );
        assert_eq!(
            states[4],
            serde_json::json!({"task":null,"kind":"note","state":"open"})
        );
    }
}

#[test]
fn task_done_and_drop_close_their_inbox_rows_through_keys_and_model_changes() {
    use bunshin_core::{
        ModelAnswer,
        day::{TaskKind, TaskOrigin},
        prompt::answer::apply_chat,
    };
    for op in ["done", "drop"] {
        let day = fixture(&[UnpromptedKind::Question]);
        let (day, _) = day
            .add(
                "synthetic task".into(),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                UnixMillis(0),
            )
            .unwrap();
        let mut data = day.data().clone();
        data.messages[0].unprompted.as_mut().unwrap().task = Some(1);
        let day = (DayFile { format: 1, data })
            .into_day(Tuning::default())
            .unwrap();
        let answer = ModelAnswer {
            json: format!(r#"{{"changes":[{{"op":"{op}","task":1}}],"reply":"synthetic"}}"#),
        };
        let outcome = apply_chat(&day, &answer, UnixMillis(60_000), Tuning::default()).unwrap();
        assert_eq!(
            outcome.day.messages()[0]
                .unprompted
                .as_ref()
                .unwrap()
                .inbox_state,
            InboxState::TaskClosed
        );
        assert!(outcome.day.inbox_view().items.is_empty());
        assert_eq!(day.inbox_view().items.len(), 1);
    }
    let day = fixture(&[UnpromptedKind::Question]);
    let (day, _) = day
        .add(
            "synthetic task".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let mut data = day.data().clone();
    data.messages[0].unprompted.as_mut().unwrap().task = Some(1);
    let day = (DayFile { format: 1, data })
        .into_day(Tuning::default())
        .unwrap();
    let (screen, _) = MainScreen::new(day, Tuning::default()).update(ScreenKey::Tab, now(0));
    let (screen, effects) = screen.update(ScreenKey::Char(' '), now(60_000));
    assert_eq!(effects, [Effect::Save]);
    assert_eq!(
        screen.day().messages()[0]
            .unprompted
            .as_ref()
            .unwrap()
            .state_changed_at,
        UnixMillis(60_000)
    );
    assert!(screen.day().inbox_view().items.is_empty());
}

#[test]
fn mute_marks_only_recent_open_items_and_preserves_old_or_future_ones() {
    for (at, expected) in [
        (899_999, InboxState::Muted),
        (900_000, InboxState::Open),
        (-1, InboxState::Open),
    ] {
        let day = fixture(&[UnpromptedKind::Question]);
        let (day, _) = day.mute(UnixMillis(3_600_000), UnixMillis(at));
        assert_eq!(
            day.messages()[0].unprompted.as_ref().unwrap().inbox_state,
            expected
        );
    }
    let day = fixture(&[UnpromptedKind::Note, UnpromptedKind::Question]);
    let (day, _) = day.react_to_inbox(Some(0), InboxAction::Close, UnixMillis(120_000));
    let (day, _) = day.mute(UnixMillis(3_600_000), UnixMillis(180_000));
    assert_eq!(
        day.messages()[0].unprompted.as_ref().unwrap().inbox_state,
        InboxState::Acknowledged
    );
    assert_eq!(
        day.messages()[1].unprompted.as_ref().unwrap().inbox_state,
        InboxState::Muted
    );
}

#[test]
fn suppressed_rows_never_enter_the_inbox_or_reaction_context() {
    use bunshin_core::day::SuppressionReason;
    let day = fixture(&[UnpromptedKind::Question, UnpromptedKind::Note]);
    let mut data = day.data().clone();
    data.messages[1].unprompted.as_mut().unwrap().suppressed = Some(SuppressionReason::SameTask);
    let day = (DayFile { format: 1, data })
        .into_day(Tuning::default())
        .unwrap();
    assert_eq!(
        day.inbox_view()
            .items
            .iter()
            .map(|item| item.message)
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(day.inbox_context(UnixMillis(120_000)).len(), 1);
    let (day, changed) = day.react_to_inbox(Some(1), InboxAction::Close, UnixMillis(120_000));
    assert!(!changed);
    assert_eq!(
        day.messages()[1].unprompted.as_ref().unwrap().inbox_state,
        InboxState::Open
    );
}

#[test]
fn invalid_or_closed_targets_are_noops_and_empty_input_keeps_an_explicit_target() {
    let day = fixture(&[UnpromptedKind::Question]);
    for target in [None, Some(u64::MAX)] {
        let (next, changed) =
            day.clone()
                .react_to_inbox(target, InboxAction::Close, UnixMillis(60_000));
        assert!(!changed);
        assert_eq!(next, day);
    }
    let (screen, _) = MainScreen::new(day, Tuning::default()).update(ScreenKey::Tab, now(0));
    let (screen, _) = screen.open_inbox().update(ScreenKey::Enter, now(0));
    for text in [String::new(), "a".repeat(401)] {
        let (next, effects) = screen.clone().submit_owner_message(&text, now(60_000));
        assert!(effects.is_empty());
        assert_eq!(next, screen);
        assert_eq!(next.reply_target(), Some(0));
    }
}

#[test]
fn go_to_task_selects_display_order_and_a_general_note_just_closes_the_overlay() {
    use bunshin_core::day::{TaskKind, TaskOrigin};
    let day = fixture(&[UnpromptedKind::Note]);
    let (day, _) = day
        .add(
            "synthetic first".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let (day, _) = day
        .add(
            "synthetic second".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    for task in [None, Some(2), Some(99)] {
        let mut data = day.data().clone();
        data.messages[0].unprompted.as_mut().unwrap().task = task;
        let day = (DayFile { format: 1, data })
            .into_day(Tuning::default())
            .unwrap();
        let (screen, _) = MainScreen::new(day, Tuning::default()).update(ScreenKey::Tab, now(0));
        let (screen, effects) = screen.open_inbox().update(ScreenKey::Char('g'), now(0));
        assert!(effects.is_empty());
        assert!(screen.inbox().is_none());
        assert_eq!(screen.focus(), Focus::Tasks);
        assert_eq!(screen.selection(), Some(usize::from(task == Some(2))));
        assert_eq!(screen.day().inbox_view().items.len(), 1);
    }
}

#[test]
fn delivered_model_notes_enter_the_inbox_with_open_state_and_save() {
    use bunshin_core::{
        Availability, CancelFlag, LanguageModel, ModelAnswer,
        checkin::{
            BatchReason, ReadyBatch,
            calls::{CallContext, CheckinCalls, CheckinEffect},
        },
        instructions::InstructionsState,
        prompt::chat::ContextExtras,
    };
    use bunshin_test_support::ScriptedLanguageModel;
    let tuning = Tuning::default();
    let day = Day::new(date(2026, 10, 4), tuning);
    let owner = InstructionsState::resolve(None, "instructions.md".into(), tuning);
    let mut calls = CheckinCalls::new(tuning);
    calls.enqueue(
        &day,
        ReadyBatch {
            reason: BatchReason::Tick,
            triggers: vec![Trigger {
                kind: TriggerKind::PlannedLook,
                task: None,
                due_at: UnixMillis(0),
            }],
        },
    );
    let context = CallContext {
        now: now(0),
        owner_waiting: false,
        input_has_text: false,
        is_tick: true,
        availability: Availability::Available,
    };
    let prepared = calls.prepare(day, &owner, ContextExtras::default(), context, |_| {
        "synthetic".into()
    });
    let request = prepared.request.unwrap();
    let model = ScriptedLanguageModel::new([Ok(ModelAnswer {
        json: r#"{"kind":"note","message":"synthetic note"}"#.into(),
    })]);
    let result = model.respond(&request.request, &CancelFlag::default());
    let posted = calls.finish(prepared.day, request.id, result, context, |_| {
        "synthetic".into()
    });
    assert_eq!(posted.effects, [CheckinEffect::Bell, CheckinEffect::Save]);
    let item = &posted.day.inbox_view().items[0];
    assert_eq!(item.kind, UnpromptedKind::Note);
    assert_eq!(item.text, "synthetic note");
    assert_eq!(
        posted.day.messages()[0]
            .unprompted
            .as_ref()
            .unwrap()
            .inbox_state,
        InboxState::Open
    );
}

#[test]
fn unavailable_model_does_not_disable_inbox_keys_and_new_days_have_no_old_items() {
    use bunshin_core::{Availability, Clock, LanguageModel, UnavailableReason};
    use bunshin_test_support::{FixedClock, ScriptedLanguageModel};
    let model = ScriptedLanguageModel::new([]).with_availability(Ok(Availability::Unavailable(
        UnavailableReason::NotInstalled,
    )));
    assert_eq!(
        model.availability(),
        Ok(Availability::Unavailable(UnavailableReason::NotInstalled))
    );
    let clock = FixedClock::at(UnixMillis(120_000), jiff::tz::Offset::UTC).unwrap();
    let previous = fixture(&[UnpromptedKind::Question, UnpromptedKind::Note]);
    let (screen, _) =
        MainScreen::new(previous.clone(), Tuning::default()).update(ScreenKey::Tab, clock.now());
    let (screen, effects) = screen
        .open_inbox()
        .update(ScreenKey::Char('X'), clock.now());
    assert_eq!(effects, [Effect::Save]);
    assert_eq!(screen.day().inbox_view().items.len(), 1);
    let next = MainScreen::new(
        Day::new(date(2026, 10, 5), Tuning::default()),
        Tuning::default(),
    );
    assert_eq!(next.day().inbox_view().header_count(), None);
    assert_eq!(next.reply_target(), None);
    assert_eq!(previous.inbox_view().items.len(), 2);
    assert!(model.requests().is_empty());
}

#[test]
fn inbox_clamps_navigation_and_closes_on_escape_or_the_opening_key() {
    let day = fixture(&[UnpromptedKind::Note, UnpromptedKind::Question]);
    let (screen, _) = MainScreen::new(day, Tuning::default()).update(ScreenKey::Tab, now(0));
    let screen = screen.open_inbox();
    let (screen, _) = screen.update(ScreenKey::Up, now(0));
    assert_eq!(screen.inbox().unwrap().selected, Some(0));
    let (screen, _) = screen.update(ScreenKey::Char('j'), now(0));
    let (screen, _) = screen.update(ScreenKey::Down, now(0));
    assert_eq!(screen.inbox().unwrap().selected, Some(1));
    let (screen, _) = screen.update(ScreenKey::Char('k'), now(0));
    assert_eq!(screen.inbox().unwrap().selected, Some(0));
    for key in [ScreenKey::Esc, ScreenKey::Char('b')] {
        let (closed, effects) = screen.clone().update(key, now(0));
        assert!(closed.inbox().is_none());
        assert!(effects.is_empty());
        assert_eq!(closed.day(), screen.day());
    }
}

#[test]
fn clock_rollback_preserves_append_order_for_inbox_replies_and_model_context() {
    let day = fixture(&[UnpromptedKind::Question; 6]);
    let mut data = day.data().clone();
    for (index, row) in data.messages.iter_mut().enumerate() {
        row.time = UnixMillis(600_000 - i64::try_from(index).unwrap() * 60_000);
    }
    data.messages[0].unprompted.as_mut().unwrap().inbox_state = InboxState::Muted;
    let day = (DayFile { format: 1, data })
        .into_day(Tuning::default())
        .unwrap();
    assert_eq!(
        day.inbox_view()
            .items
            .iter()
            .map(|item| item.message)
            .collect::<Vec<_>>(),
        [5, 4, 3, 2, 1]
    );
    assert_eq!(day.implicit_reply_target(UnixMillis(660_000)), Some(5));
    let day = day.record_owner_message("synthetic rollback reply", None, UnixMillis(660_000));
    assert_eq!(day.messages().last().unwrap().answers_question, Some(5));
    assert_eq!(
        day.inbox_context(UnixMillis(660_000))
            .iter()
            .map(|item| item.state)
            .collect::<Vec<_>>(),
        [
            InboxState::Open,
            InboxState::Open,
            InboxState::Open,
            InboxState::Open,
            InboxState::Answered
        ]
    );
}
