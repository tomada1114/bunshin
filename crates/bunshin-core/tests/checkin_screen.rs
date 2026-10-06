//! The screen's check-ins (§3.5, §3.7): the open, the tick, the shared worker, the bell,
//! and the header items a front end draws.
use bunshin_core::{
    Availability, ModelAnswer, ModelError, Now, Tuning, UnavailableReason, UnixMillis,
    checkin::{calls::FixedDeadline, plan_look},
    day::{
        Day, MessageKind, TaskKind, TaskOrigin, TriggerKind, UnpromptedKind,
        store::{DayStore, DayStoreError},
    },
    instructions::InstructionsState,
    screen::{
        ChatNotice, CheckinHeader, Effect, MainScreen, ScreenKey,
        help::{inbox_help, leftovers_help},
        keys::ScreenAction,
    },
};
use bunshin_test_support::InMemoryDayStore;
use jiff::civil::{Date, date, time};
use std::path::PathBuf;

const DAY: Date = date(2026, 10, 2);

/// A reading `seconds` after midnight of the fixture day; instants keep the same offset.
fn at(seconds: i64) -> Now {
    let hours = i8::try_from(seconds / 3_600).unwrap_or_else(|_| panic!("hour"));
    let minutes = i8::try_from(seconds / 60 % 60).unwrap_or_else(|_| panic!("minute"));
    let secs = i8::try_from(seconds % 60).unwrap_or_else(|_| panic!("second"));
    Now {
        instant: UnixMillis(1_000_000_000 + seconds * 1_000),
        local: DAY.at(hours, minutes, secs, 0),
    }
}
fn hm(hour: i64, minute: i64) -> Now {
    at(hour * 3_600 + minute * 60)
}
fn owner() -> InstructionsState {
    InstructionsState::resolve(
        Some("秘書"),
        PathBuf::from("instructions.md"),
        Tuning::default(),
    )
}
fn render(note: &FixedDeadline) -> String {
    format!("fixed:{}", note.task.number)
}
fn deadline_day(titles: &[(&str, i8)]) -> Day {
    let mut day = Day::new(DAY, Tuning::default());
    for (title, hour) in titles {
        day = match day.add(
            (*title).into(),
            TaskKind::Deadline,
            Some(time(*hour, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0),
        ) {
            Ok((day, _)) => day,
            Err(error) => panic!("fixture task: {error:?}"),
        };
    }
    day
}
/// The binary's controller without a terminal; returns the bells it would ring.
fn persist(
    screen: MainScreen,
    effects: &[Effect],
    store: &dyn DayStore,
    now: Now,
) -> (MainScreen, usize) {
    let mut screen = screen;
    let mut bells = 0;
    for effect in effects {
        match effect {
            Effect::Save => {
                let result = store.save(screen.day());
                screen = screen.record_save_result(result, now.instant, "");
            }
            Effect::SaveLeftovers => {
                let result: Result<(), DayStoreError> =
                    screen.leftovers_day().map_or(Ok(()), |day| store.save(day));
                screen = screen.record_save_result(result, now.instant, "");
            }
            Effect::StartDay => {
                let (next, effects) = screen.start_day(store, now);
                let (next, more) = persist(next, &effects, store, now);
                screen = next;
                bells += more;
            }
            Effect::Bell => bells += 1,
            Effect::Quit | Effect::CancelModel | Effect::ChatNotice(_) => {}
        }
    }
    (screen, bells)
}
/// Open the screen and run its day start, as the loop does before the first frame.
fn opened(day: Day, now: Now, store: &InMemoryDayStore) -> MainScreen {
    let (screen, effects) = MainScreen::new(day, Tuning::default()).open_checkins(now);
    persist(screen, &effects, store, now).0
}
fn available(screen: MainScreen, now: Now) -> MainScreen {
    screen
        .record_availability(Ok(Availability::Available), now.instant)
        .0
}
fn answer(json: &str) -> ModelAnswer {
    ModelAnswer { json: json.into() }
}

#[test]
fn opening_runs_the_day_start_once_and_its_note_rings_once_after_the_first_probe() {
    let now = hm(9, 0);
    let store = InMemoryDayStore::new(Tuning::default());
    let (screen, effects) =
        MainScreen::new(Day::new(DAY, Tuning::default()), Tuning::default()).open_checkins(now);
    assert_eq!(effects, vec![Effect::StartDay]);
    let (again, effects) = screen.clone().open_checkins(now);
    assert!(effects.is_empty());
    assert_eq!(again, screen);
    let (screen, _) = persist(screen, &[Effect::StartDay], &store, now);
    assert!(screen.checkin_needs_instructions());
    let (screen, request, effects) = screen.prepare_checkin(&owner(), now, render);
    assert_eq!((request, effects), (None, Vec::new()));
    let screen = available(screen, now);
    let (screen, request, _) = screen.prepare_checkin(&owner(), now, render);
    let request = request.unwrap();
    assert!(!screen.checkin_needs_instructions());
    assert!(screen.checkin_header(now).checking_in);
    let (screen, effects) = screen.finish_checkin(
        request.id,
        Ok(answer(
            r#"{"kind":"note","message":"おはよう","next_look_minutes":60}"#,
        )),
        now,
        render,
    );
    assert_eq!(effects, vec![Effect::Bell, Effect::Save]);
    let header = screen.checkin_header(now);
    assert!(!header.checking_in);
    assert_eq!(header.inbox, Some(1));
    assert_eq!(header.next_look, Some(DAY.at(10, 0, 0, 0)));
    let posted = screen.day().messages().last().unwrap();
    assert_eq!(posted.kind, MessageKind::Unprompted);
    let extra = posted.unprompted.as_ref().unwrap();
    assert_eq!(
        (extra.kind, extra.trigger.kind),
        (UnpromptedKind::Note, TriggerKind::DayStart)
    );
    let (stale, effects) = screen.clone().finish_checkin(
        request.id,
        Ok(answer(
            r#"{"kind":"note","message":"重複","next_look_minutes":60}"#,
        )),
        now,
        render,
    );
    assert!(effects.is_empty());
    assert_eq!(stale, screen);
}

#[test]
fn an_idle_tick_makes_no_call_and_no_write() {
    let now = hm(9, 0);
    let store = InMemoryDayStore::new(Tuning::default());
    let screen = available(opened(Day::new(DAY, Tuning::default()), now, &store), now);
    let (screen, request, _) = screen.prepare_checkin(&owner(), now, render);
    let (mut screen, effects) = screen.finish_checkin(
        request.unwrap().id,
        Ok(answer(
            r#"{"kind":"silent","message":"","next_look_minutes":60}"#,
        )),
        now,
        render,
    );
    assert_eq!(effects, vec![Effect::Save]);
    for second in [1, 59, 60, 61, 120, 600] {
        let now = at(9 * 3_600 + second);
        let (next, effects) = screen.tick(now);
        assert!(effects.is_empty(), "tick at +{second}s: {effects:?}");
        assert!(!next.checkin_needs_instructions());
        let (next, request, effects) = next.prepare_checkin(&owner(), now, render);
        assert_eq!((request, effects), (None, Vec::new()), "+{second}s");
        screen = next;
    }
}

#[test]
fn two_fixed_deadline_notes_in_one_dispatch_ring_twice_while_the_model_is_unavailable() {
    let now = hm(14, 30);
    let store = InMemoryDayStore::new(Tuning::default());
    let screen = opened(
        deadline_day(&[("資料作成", 14), ("請求書", 13)]),
        now,
        &store,
    )
    .record_availability(
        Ok(Availability::Unavailable(UnavailableReason::NotInstalled)),
        now.instant,
    )
    .0;
    // While unavailable, fixed notes wait for the tick interval rather than every wake.
    let (screen, request, effects) = screen.prepare_checkin(&owner(), now, render);
    assert_eq!((request, effects), (None, Vec::new()));
    let later = at(14 * 3_600 + 30 * 60 + 60);
    let (screen, _) = screen.tick(later);
    let (screen, request, effects) = screen.prepare_checkin(&owner(), later, render);
    assert_eq!(request, None);
    assert_eq!(effects, vec![Effect::Bell, Effect::Bell, Effect::Save]);
    let texts = screen
        .day()
        .messages()
        .iter()
        .filter(|row| {
            row.unprompted
                .as_ref()
                .is_some_and(|extra| extra.suppressed.is_none())
        })
        .map(|row| row.text.clone())
        .collect::<Vec<_>>();
    assert_eq!(texts, vec!["fixed:1", "fixed:2"]);
    assert_eq!(screen.checkin_header(later).inbox, Some(2));
}

#[test]
fn typing_holds_the_day_start_and_the_header_counts_it_until_the_input_clears() {
    let now = hm(14, 30);
    let store = InMemoryDayStore::new(Tuning::default());
    let screen = MainScreen::new(Day::new(DAY, Tuning::default()), Tuning::default())
        .update(ScreenKey::Char('あ'), now)
        .0;
    let (screen, effects) = screen.open_checkins(now);
    let (screen, _) = persist(screen, &effects, &store, now);
    let screen = available(screen, now);
    assert_eq!(screen.checkin_header(now).held, Some(1));
    let (screen, request, _) = screen.prepare_checkin(&owner(), now, render);
    assert_eq!(request, None);
    let (screen, effects) = screen.tick(at(14 * 3_600 + 30 * 60 + 1));
    assert!(effects.is_empty());
    assert_eq!(screen.checkin_header(now).held, Some(1));
    let screen = screen.update(ScreenKey::Esc, now).0;
    assert_eq!(screen.input().text(), "");
    assert_eq!(screen.checkin_header(now).held, None);
    let wake = at(14 * 3_600 + 30 * 60 + 2);
    let (screen, _) = screen.tick(wake);
    assert!(screen.checkin_needs_instructions());
    let (screen, request, _) = screen.prepare_checkin(&owner(), wake, render);
    assert!(request.is_some());
    assert!(screen.checkin_header(wake).checking_in);
}

#[test]
fn a_held_answer_rings_only_after_the_owner_sends_or_clears() {
    let now = hm(9, 0);
    let store = InMemoryDayStore::new(Tuning::default());
    let screen = available(opened(Day::new(DAY, Tuning::default()), now, &store), now);
    let (screen, request, _) = screen.prepare_checkin(&owner(), now, render);
    let screen = screen.update(ScreenKey::Char('a'), now).0;
    let (screen, effects) = screen.finish_checkin(
        request.unwrap().id,
        Ok(answer(
            r#"{"kind":"question","message":"今日の予定は？","next_look_minutes":60}"#,
        )),
        now,
        render,
    );
    assert!(!effects.contains(&Effect::Bell));
    assert_eq!(screen.checkin_header(now).held, Some(1));
    let (screen, request, effects) = screen.prepare_checkin(&owner(), now, render);
    assert_eq!(request, None);
    assert!(!effects.contains(&Effect::Bell));
    let screen = screen.update(ScreenKey::Esc, now).0;
    let (screen, request, effects) = screen.prepare_checkin(&owner(), now, render);
    assert_eq!(request, None);
    assert_eq!(effects, vec![Effect::Bell, Effect::Save]);
    assert_eq!(screen.checkin_header(now).inbox, Some(1));
}

#[test]
fn an_unavailable_check_in_result_records_the_model_state_once() {
    let now = hm(9, 0);
    let store = InMemoryDayStore::new(Tuning::default());
    let screen = available(opened(Day::new(DAY, Tuning::default()), now, &store), now);
    let (screen, request, _) = screen.prepare_checkin(&owner(), now, render);
    let (screen, effects) = screen.finish_checkin(
        request.unwrap().id,
        Err(ModelError::Unavailable(UnavailableReason::NotInstalled)),
        now,
        render,
    );
    assert_eq!(
        screen.model_availability(),
        Some(Availability::Unavailable(UnavailableReason::NotInstalled))
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(
                effect,
                Effect::ChatNotice(ChatNotice::Unavailable(UnavailableReason::NotInstalled))
            ))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| **effect == Effect::Save)
            .count(),
        1
    );
    assert!(!effects.contains(&Effect::Bell));
}

#[test]
fn ticks_wait_for_the_day_start_stop_at_the_boundary_and_queue_the_evening_review() {
    let store = InMemoryDayStore::new(Tuning::default());
    let closed = MainScreen::new(Day::new(DAY, Tuning::default()), Tuning::default());
    let (closed, effects) = closed.tick(hm(18, 0));
    assert!(effects.is_empty());
    assert!(closed.checkin().is_none());
    let screen = opened(Day::new(DAY, Tuning::default()), hm(17, 59), &store);
    let screen = screen.take_checkin_batches().0;
    let (screen, effects) = screen.tick(hm(18, 0));
    assert_eq!(effects, vec![Effect::Save]);
    assert!(screen.checkin_needs_instructions());
    let (_, batches) = screen.clone().take_checkin_batches();
    assert!(batches.iter().any(|batch| {
        batch
            .triggers
            .iter()
            .any(|t| t.kind == TriggerKind::EveningReview)
    }));
    let next_morning = Now {
        instant: UnixMillis(hm(18, 0).instant.0 + 10 * 3_600_000),
        local: date(2026, 10, 3).at(4, 0, 0, 0),
    };
    let (_, effects) = screen.tick(next_morning);
    assert!(effects.is_empty());
}

#[test]
fn the_header_reports_mute_active_hours_and_the_planned_look() {
    let now = hm(15, 31);
    let screen = MainScreen::new(Day::new(DAY, Tuning::default()), Tuning::default())
        .update(ScreenKey::Tab, now)
        .0;
    assert_eq!(
        screen.checkin_header(now),
        CheckinHeader {
            inbox: None,
            held: None,
            muted_until: None,
            outside_hours_until: None,
            next_look: None,
            checking_in: false,
        }
    );
    assert_eq!(screen.tuning(), Tuning::default());
    let (muted, effects) = screen.update(ScreenKey::Char('m'), now);
    assert_eq!(effects, vec![Effect::Save]);
    let end = UnixMillis(now.instant.0 + 3_600_000);
    assert_eq!(muted.checkin_header(now).muted_until, Some(end));
    assert_eq!(
        muted.checkin_header(at(16 * 3_600 + 31 * 60)).muted_until,
        None
    );
    let (unmuted, _) = muted.update(ScreenKey::Char('m'), now);
    assert_eq!(unmuted.checkin_header(now).muted_until, None);
    for (now, outside) in [
        (at(8 * 3_600 - 1), true),
        (hm(8, 0), false),
        (at(22 * 3_600 - 1), false),
        (hm(22, 0), true),
        (hm(23, 10), true),
    ] {
        assert_eq!(
            unmuted.checkin_header(now).outside_hours_until,
            outside.then_some(time(8, 0, 0, 0)),
            "{}",
            now.local
        );
    }
    let planned = MainScreen::new(
        plan_look(
            Day::new(DAY, Tuning::default()),
            now,
            Some(29),
            Tuning::default(),
        ),
        Tuning::default(),
    );
    assert_eq!(
        planned.checkin_header(now).next_look,
        Some(DAY.at(16, 0, 0, 0))
    );
}

#[test]
fn b_opens_the_inbox_only_from_the_task_pane_and_its_line_comes_from_the_table() {
    let now = hm(9, 0);
    let screen = MainScreen::new(Day::new(DAY, Tuning::default()), Tuning::default());
    let typed = screen.clone().update(ScreenKey::Char('b'), now).0;
    assert_eq!(typed.inbox(), None);
    assert_eq!(typed.input().text(), "b");
    let tasks = screen.update(ScreenKey::Tab, now).0;
    let open = tasks.update(ScreenKey::Char('b'), now).0;
    assert!(open.inbox().is_some_and(|view| view.items.is_empty()));
    for close in [ScreenKey::Esc, ScreenKey::Char('b'), ScreenKey::Char('ｂ')] {
        assert_eq!(open.clone().update(close, now).0.inbox(), None, "{close:?}");
    }
    assert_eq!(
        inbox_help()
            .iter()
            .map(|binding| binding.action)
            .collect::<Vec<_>>(),
        vec![
            ScreenAction::InboxRespond,
            ScreenAction::InboxClose,
            ScreenAction::InboxAcknowledgeNotes,
            ScreenAction::InboxTask,
            ScreenAction::CloseInbox,
        ]
    );
    assert_eq!(
        leftovers_help()
            .iter()
            .map(|binding| binding.action)
            .collect::<Vec<_>>(),
        vec![
            ScreenAction::Help,
            ScreenAction::CarryLeftover,
            ScreenAction::DropLeftover,
            ScreenAction::CarryAllLeftovers,
            ScreenAction::DropAllLeftovers,
            ScreenAction::MoveFocus,
        ]
    );
}
