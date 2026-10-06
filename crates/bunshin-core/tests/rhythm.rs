//! The day's rhythm (§3.6): day start, leftovers, yesterday's record, evening review.
use bunshin_core::{
    Availability, ModelAnswer, Now, Tuning, UnavailableReason, UnixMillis,
    checkin::{
        BatchReason, Checkin,
        calls::{CallContext, CheckinCalls, FixedDeadline},
    },
    day::{
        Day, LeftoverDecision, TaskKind, TaskOrigin, TaskStatus, Trigger, TriggerKind,
        file::DayFile,
        store::{DayStore, DayStoreError},
    },
    instructions::InstructionsState,
    prompt::{
        budget::estimate,
        chat::{ContextExtras, build_chat},
        checkin::build_checkin,
        yesterday::yesterday_text,
    },
    rhythm::{self, record::day_record},
    screen::{Effect, Focus, MainScreen, ScreenError, ScreenKey},
};
use bunshin_test_support::InMemoryDayStore;
use jiff::{
    civil::{Date, date, time},
    tz::TimeZone,
};
use std::path::PathBuf;

fn at(day: Date, hour: i8, minute: i8) -> Now {
    let local = day.at(hour, minute, 0, 0);
    let instant = local.to_zoned(TimeZone::UTC).map_or_else(
        |error| panic!("fixture instant: {error}"),
        |zoned| zoned.timestamp().as_millisecond(),
    );
    Now {
        instant: UnixMillis(instant),
        local,
    }
}
fn oct(day: i8) -> Date {
    date(2026, 10, day)
}
fn add(day: Day, title: &str, kind: TaskKind, hour: Option<i8>) -> Day {
    match day.add(
        title.into(),
        kind,
        hour.map(|hour| time(hour, 0, 0, 0)),
        TaskOrigin::Key,
        UnixMillis(0),
    ) {
        Ok((day, _)) => day,
        Err(error) => panic!("fixture task: {error:?}"),
    }
}
fn done(day: Day, number: u64) -> Day {
    match day.done(number, UnixMillis(0)) {
        Ok((day, _)) => day,
        Err(error) => panic!("fixture done: {error:?}"),
    }
}
/// The issue's example: 10/1 with three done, a deadline, an untimed task, an appointment.
fn october_first() -> Day {
    let mut day = Day::new(oct(1), Tuning::default());
    for title in ["資料作成", "メール返信", "定例"] {
        day = add(day, title, TaskKind::Untimed, None);
    }
    for number in 1..=3 {
        day = done(day, number);
    }
    day = add(day, "経費精算", TaskKind::Deadline, Some(17));
    day = add(day, "請求書の確認", TaskKind::Untimed, None);
    add(day, "ジム", TaskKind::Appointment, Some(18))
}
fn with_held_trigger(day: &Day) -> Day {
    let mut data = day.data().clone();
    data.held_triggers.push(Trigger {
        kind: TriggerKind::PlannedLook,
        task: None,
        due_at: UnixMillis(1),
    });
    match (DayFile { format: 1, data }).into_day(Tuning::default()) {
        Ok(day) => day,
        Err(error) => panic!("fixture held trigger: {error:?}"),
    }
}
fn store_with(days: &[Day]) -> InMemoryDayStore {
    let store = InMemoryDayStore::new(Tuning::default());
    for day in days {
        if let Err(error) = store.save(day) {
            panic!("fixture save: {error:?}");
        }
    }
    store
}
/// The binary's controller, without a terminal: saves run before the next key.
fn persist(screen: MainScreen, effects: &[Effect], store: &dyn DayStore, now: Now) -> MainScreen {
    let mut screen = screen;
    for effect in effects {
        match effect {
            Effect::Save => {
                let result = store.save(screen.day());
                screen = screen.record_save_result(result, now.instant, "");
            }
            Effect::SaveLeftovers => {
                let result = screen.leftovers_day().map_or(Ok(()), |day| store.save(day));
                screen = screen.record_save_result(result, now.instant, "");
            }
            Effect::StartDay => {
                let (next, effects) = screen.start_day(store, now);
                screen = persist(next, &effects, store, now);
            }
            Effect::Quit | Effect::CancelModel | Effect::ChatNotice(_) | Effect::Bell => {}
        }
    }
    screen
}
fn opened(store: &InMemoryDayStore, now: Now) -> (MainScreen, Vec<Effect>) {
    let tuning = Tuning::default();
    let date = bunshin_core::logical_date(now.local, tuning.day_boundary);
    let day = store
        .load(date)
        .unwrap_or_else(|error| panic!("fixture load: {error:?}"));
    MainScreen::new(day, tuning).start_day(store, now)
}
fn titles(screen: &MainScreen) -> Vec<String> {
    screen
        .leftovers()
        .into_iter()
        .map(|task| task.title)
        .collect()
}
fn owner() -> InstructionsState {
    InstructionsState::resolve(
        Some("秘書"),
        PathBuf::from("instructions.md"),
        Tuning::default(),
    )
}
fn context(now: Now, availability: Availability) -> CallContext {
    CallContext {
        now,
        owner_waiting: false,
        input_has_text: false,
        is_tick: true,
        availability,
    }
}
fn fixed(_: &FixedDeadline) -> String {
    String::from("fixed")
}
fn tab(screen: MainScreen, now: Now) -> MainScreen {
    screen.update(ScreenKey::Tab, now).0
}

#[test]
fn rhythm_first_open_offers_open_deadline_and_untimed_leftovers_and_writes_yesterdays_record() {
    let store = store_with(&[with_held_trigger(&october_first())]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    assert_eq!(effects, vec![Effect::Save, Effect::SaveLeftovers]);
    assert_eq!(screen.day().date(), oct(2));
    assert_eq!(titles(&screen), ["経費精算", "請求書の確認"]);
    assert_eq!(screen.leftover_selection(), Some(0));
    assert_eq!(screen.selection(), None, "the cursor starts in the block");
    let record = screen.day().data().yesterday_record.clone().unwrap();
    assert_eq!(record.date, oct(1));
    assert_eq!(
        record.text,
        "2026-10-01の記録: 完了3件（資料作成、メール返信、定例）、持ち越し0件、やめた0件、未完了3件（経費精算、ジム、請求書の確認）"
    );
    assert!(
        screen
            .leftovers_day()
            .unwrap()
            .data()
            .held_triggers
            .is_empty(),
        "the previous day's held triggers are dropped at the day start"
    );
    let screen = persist(screen, &effects, &store, now);
    assert!(store.load(oct(1)).unwrap().data().held_triggers.is_empty());
    assert_eq!(
        store.load(oct(2)).unwrap().data().yesterday_record,
        Some(record)
    );
    let (_, batches) = screen.take_checkin_batches();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].reason, BatchReason::DayStart);
    assert_eq!(batches[0].triggers.len(), 1);
    assert_eq!(batches[0].triggers[0].kind, TriggerKind::DayStart);
}

#[test]
fn rhythm_day_start_is_held_by_neither_active_hours_the_gap_nor_the_mute() {
    let tuning = Tuning::default();
    let now = at(oct(2), 5, 0);
    let mut data = Day::new(oct(2), tuning).data().clone();
    data.muted_until = Some(UnixMillis(now.instant.0 + 3_600_000));
    data.last_unprompted_at = Some(now.instant);
    let today = (DayFile { format: 1, data }).into_day(tuning).unwrap();
    let started = rhythm::start_day(
        today,
        Some(october_first()),
        Checkin::new(now, tuning),
        now,
        false,
        tuning,
    );
    let ready = started.ready.expect("exempt from every delivery guard");
    assert_eq!(ready.reason, BatchReason::DayStart);
    assert_eq!(ready.triggers[0].kind, TriggerKind::DayStart);
    assert!(started.save_day);
    assert!(!started.save_previous, "nothing was held on 10/1");
    assert!(rhythm::day_started(&started.day));
}

#[test]
fn rhythm_day_start_runs_once_per_logical_day_and_a_reopen_only_shows_its_leftovers() {
    let store = store_with(&[october_first()]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    let screen = persist(screen, &effects, &store, now);
    assert_eq!(screen.take_checkin_batches().1.len(), 1);
    let later = at(oct(2), 11, 0);
    let (reopened, effects) = opened(&store, later);
    assert!(
        effects.is_empty(),
        "no second day start, so nothing to save"
    );
    assert!(reopened.take_checkin_batches().1.is_empty());
    let (reopened, _) = opened(&store, later);
    assert_eq!(titles(&reopened), ["経費精算", "請求書の確認"]);
}

#[test]
fn rhythm_opening_at_three_fifty_nine_stays_on_the_previous_logical_day() {
    let store = store_with(&[october_first()]);
    let evening = at(oct(1), 9, 0);
    let (screen, effects) = opened(&store, evening);
    let screen = persist(screen, &effects, &store, evening);
    let screen = screen.take_checkin_batches().0;
    let night = at(oct(2), 3, 59);
    let (screen, effects) = screen.start_day(&store, night);
    assert_eq!(screen.day().date(), oct(1));
    assert_eq!(
        effects,
        vec![Effect::Save],
        "only 10/1's evening review, held outside active hours"
    );
    let starts = screen
        .day()
        .data()
        .triggers_fired
        .iter()
        .filter(|trigger| trigger.kind == TriggerKind::DayStart)
        .count();
    assert_eq!(starts, 1, "no second day start");
    assert!(screen.take_checkin_batches().1.is_empty());
}

#[test]
fn rhythm_a_screen_left_open_across_four_starts_the_day_at_the_first_key_only() {
    let store = store_with(&[october_first()]);
    let evening = at(oct(1), 23, 0);
    let (screen, effects) = opened(&store, evening);
    let screen = persist(screen, &effects, &store, evening);
    let (screen, _) = screen.take_checkin_batches();
    let dawn = at(oct(2), 4, 1);
    let (screen, effects) = screen.check_evening_review(dawn);
    assert!(effects.is_empty(), "nothing happens at the boundary itself");
    let (screen, batches) = screen.take_checkin_batches();
    assert!(batches.is_empty());
    assert_eq!(screen.day().date(), oct(1));
    let (screen, effects) = screen.update(ScreenKey::Char('j'), dawn);
    assert_eq!(effects, vec![Effect::StartDay]);
    assert_eq!(screen.day().date(), oct(1), "the key itself is consumed");
    let screen = persist(screen, &effects, &store, dawn);
    assert_eq!(screen.day().date(), oct(2));
    assert_eq!(screen.leftovers_day().unwrap().date(), oct(1));
    assert_eq!(titles(&screen), ["経費精算", "請求書の確認"]);
    let (_, batches) = screen.take_checkin_batches();
    assert_eq!(batches[0].triggers[0].kind, TriggerKind::DayStart);
}

#[test]
fn rhythm_an_open_task_form_holds_the_turnover_until_it_closes() {
    let store = store_with(&[october_first()]);
    let evening = at(oct(1), 23, 0);
    let (screen, effects) = opened(&store, evening);
    let screen = persist(screen, &effects, &store, evening);
    let (screen, _) = screen.update(ScreenKey::Tab, evening);
    let (screen, _) = screen.update(ScreenKey::Char('a'), evening);
    assert!(screen.form().is_some());
    let dawn = at(oct(2), 4, 1);
    let (screen, effects) = screen.update(ScreenKey::Char('x'), dawn);
    assert!(
        !effects.contains(&Effect::StartDay),
        "typing in the form is kept"
    );
    assert!(screen.form().is_some());
    assert_eq!(screen.day().date(), oct(1));
    let (screen, _) = screen.update(ScreenKey::Esc, dawn);
    assert!(screen.form().is_none());
    let (_, effects) = screen.update(ScreenKey::Char('j'), dawn);
    assert_eq!(effects, vec![Effect::StartDay]);
}

#[test]
fn rhythm_quit_still_works_after_the_boundary_without_starting_the_day() {
    let store = store_with(&[]);
    let evening = at(oct(1), 23, 0);
    let (screen, effects) = opened(&store, evening);
    let screen = persist(screen, &effects, &store, evening);
    let (screen, effects) = screen.update(ScreenKey::Interrupt, at(oct(2), 4, 30));
    assert_eq!(effects, vec![Effect::Quit]);
    assert!(screen.finished());
    assert_eq!(screen.day().date(), oct(1));
}

#[test]
fn rhythm_a_screen_never_started_does_not_turn_over() {
    let tuning = Tuning::default();
    let screen = MainScreen::new(Day::new(oct(1), tuning), tuning);
    let (screen, effects) = screen.update(ScreenKey::Tab, at(oct(2), 9, 0));
    assert!(effects.is_empty());
    assert_eq!(screen.focus(), Focus::Tasks);
    assert!(screen.checkin().is_none());
}

#[test]
fn rhythm_no_previous_day_means_no_leftovers_and_a_day_start_that_asks_for_the_plan() {
    let store = store_with(&[]);
    let now = at(oct(2), 9, 0);
    let (screen, effects) = opened(&store, now);
    assert_eq!(effects, vec![Effect::Save]);
    assert!(screen.leftovers().is_empty());
    assert_eq!(screen.leftover_selection(), None);
    assert_eq!(screen.day().data().yesterday_record, None);
    let (_, batches) = screen.take_checkin_batches();
    assert_eq!(batches[0].triggers[0].kind, TriggerKind::DayStart);
}

#[test]
fn rhythm_the_last_day_on_record_supplies_the_leftovers_however_long_ago() {
    let mut old = Day::new(date(2026, 9, 27), Tuning::default());
    old = add(old, "古い残り", TaskKind::Untimed, None);
    let store = store_with(&[old]);
    let (screen, _) = opened(&store, at(oct(2), 9, 0));
    assert_eq!(titles(&screen), ["古い残り"]);
    assert_eq!(
        screen.day().data().yesterday_record.as_ref().unwrap().date,
        date(2026, 9, 27)
    );
}

#[test]
fn rhythm_a_store_failure_keeps_the_day_and_is_not_retried_on_every_key() {
    let store = store_with(&[]);
    store.seed_error(oct(1), DayStoreError::Unreadable);
    let tuning = Tuning::default();
    let (screen, effects) =
        MainScreen::new(Day::new(oct(2), tuning), tuning).start_day(&store, at(oct(2), 9, 0));
    assert!(effects.is_empty());
    assert_eq!(
        screen.error(),
        Some(ScreenError::Store(DayStoreError::Unreadable))
    );
    assert!(screen.checkin().is_none());
    let healthy = store_with(&[]);
    let (screen, effects) = opened(&healthy, at(oct(2), 23, 0));
    let screen = persist(screen, &effects, &healthy, at(oct(2), 23, 0));
    healthy.seed_error(oct(3), DayStoreError::Unreadable);
    let dawn = at(oct(3), 5, 0);
    let (screen, effects) = screen.update(ScreenKey::Tab, dawn);
    assert_eq!(effects, vec![Effect::StartDay]);
    let screen = persist(screen, &effects, &healthy, dawn);
    assert_eq!(screen.day().date(), oct(2));
    assert_eq!(
        screen.error(),
        Some(ScreenError::Store(DayStoreError::Unreadable))
    );
    let (screen, effects) = screen.update(ScreenKey::Tab, dawn);
    assert!(effects.is_empty(), "keys keep working on the loaded day");
    assert_eq!(screen.focus(), Focus::Tasks);
}

#[test]
fn rhythm_day_start_check_in_carries_the_leftovers_and_yesterdays_record_to_the_model() {
    let store = store_with(&[october_first()]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    let screen = persist(screen, &effects, &store, now);
    let leftovers = screen.leftovers();
    let (screen, batches) = screen.take_checkin_batches();
    let mut calls = CheckinCalls::new(Tuning::default());
    for batch in batches {
        calls.enqueue(screen.day(), batch);
    }
    let update = calls.prepare(
        screen.day().clone(),
        &owner(),
        ContextExtras {
            leftovers: &leftovers,
            ..ContextExtras::default()
        },
        context(now, Availability::Available),
        fixed,
    );
    let request = update.request.expect("the day-start check-in is sent");
    let prompt: serde_json::Value = serde_json::from_str(&request.request.prompt).unwrap();
    assert_eq!(prompt["triggers"], serde_json::json!([["s", null]]));
    assert_eq!(
        prompt["leftovers"],
        serde_json::json!([
            {"number": 4, "title": "経費精算", "kind": "deadline", "time": "17:00"},
            {"number": 5, "title": "請求書の確認", "kind": "untimed", "time": null},
        ])
    );
    assert!(
        prompt["yesterday"]
            .as_str()
            .unwrap()
            .starts_with("2026-10-01の記録: 完了3件")
    );
    assert!(request.request.instructions.contains("今日の予定を尋ねる"));
}

#[test]
fn rhythm_leftovers_by_key_work_while_the_model_is_unavailable_and_the_greeting_waits() {
    let store = store_with(&[october_first()]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    let screen = persist(screen, &effects, &store, now);
    let (screen, batches) = screen.take_checkin_batches();
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(screen.day(), batches[0].clone());
    let update = calls.prepare(
        screen.day().clone(),
        &owner(),
        ContextExtras::default(),
        context(
            now,
            Availability::Unavailable(UnavailableReason::NotInstalled),
        ),
        fixed,
    );
    assert!(update.request.is_none());
    assert!(
        update
            .day
            .data()
            .held_triggers
            .iter()
            .any(|trigger| trigger.kind == TriggerKind::DayStart),
        "the greeting waits for the model"
    );
    let screen = tab(screen, now);
    let (screen, effects) = screen.update(ScreenKey::Char('C'), now);
    assert_eq!(effects, vec![Effect::Save, Effect::SaveLeftovers]);
    assert!(screen.leftovers().is_empty());
}

#[test]
fn rhythm_key_c_in_the_block_carries_every_leftover_as_one_change_set_and_undo_restores_both_days()
{
    let store = store_with(&[october_first()]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    let screen = tab(persist(screen, &effects, &store, now), now);
    let (screen, effects) = screen.update(ScreenKey::Char('C'), now);
    assert_eq!(effects, vec![Effect::Save, Effect::SaveLeftovers]);
    let screen = persist(screen, &effects, &store, now);
    let today = store.load(oct(2)).unwrap();
    let carried = today
        .tasks()
        .iter()
        .map(|task| {
            (
                task.number,
                task.title.as_str(),
                task.kind,
                task.origin.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        carried,
        [
            (
                1,
                "経費精算",
                TaskKind::Untimed,
                TaskOrigin::CarriedOver { date: oct(1) }
            ),
            (
                2,
                "請求書の確認",
                TaskKind::Untimed,
                TaskOrigin::CarriedOver { date: oct(1) }
            ),
        ]
    );
    let earlier = store.load(oct(1)).unwrap();
    let statuses = earlier
        .tasks()
        .iter()
        .map(|task| task.status)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses,
        [
            TaskStatus::Done,
            TaskStatus::Done,
            TaskStatus::Done,
            TaskStatus::CarriedOver,
            TaskStatus::CarriedOver,
            TaskStatus::Open,
        ],
        "the appointment stays open on its own day"
    );
    assert!(screen.leftovers().is_empty(), "the block closes");
    assert_eq!(screen.leftover_selection(), None);
    let change = screen.last_change().unwrap();
    assert_eq!(change.changes.len(), 2);
    assert_eq!(change.leftovers.len(), 2);
    assert_eq!(
        today
            .messages()
            .iter()
            .filter(|message| message.change_set.is_some())
            .count(),
        1,
        "one change line"
    );

    let (screen, effects) = screen.update(ScreenKey::Undo, now);
    assert_eq!(effects, vec![Effect::Save, Effect::SaveLeftovers]);
    let screen = persist(screen, &effects, &store, now);
    assert!(store.load(oct(2)).unwrap().tasks().is_empty());
    assert_eq!(
        store
            .load(oct(1))
            .unwrap()
            .tasks()
            .iter()
            .filter(|task| task.status == TaskStatus::Open)
            .count(),
        3
    );
    assert_eq!(titles(&screen), ["経費精算", "請求書の確認"]);
    assert!(screen.last_change().unwrap().undo);
    let reread = DayFile::from(&store.load(oct(2)).unwrap());
    assert_eq!(reread.data.next_task_number, 3, "carried numbers stay used");
}

#[test]
fn rhythm_key_c_carries_the_selected_leftover_with_the_next_number_and_d_drops_one() {
    let mut today = Day::new(oct(2), Tuning::default());
    today = add(today, "今日の作業", TaskKind::Untimed, None);
    let store = store_with(&[october_first(), today]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    let screen = tab(persist(screen, &effects, &store, now), now);
    let screen = screen.update(ScreenKey::Down, now).0;
    assert_eq!(screen.leftover_selection(), Some(1));
    let (screen, effects) = screen.update(ScreenKey::Char('c'), now);
    let screen = persist(screen, &effects, &store, now);
    let added = screen.day().tasks().last().unwrap();
    assert_eq!((added.number, added.title.as_str()), (2, "請求書の確認"));
    assert_eq!(screen.leftover_selection(), Some(0));
    let (screen, effects) = screen.update(ScreenKey::Char('d'), now);
    assert_eq!(effects, vec![Effect::Save, Effect::SaveLeftovers]);
    let screen = persist(screen, &effects, &store, now);
    assert_eq!(screen.day().tasks().len(), 2, "a drop adds nothing today");
    assert_eq!(
        store.load(oct(1)).unwrap().tasks()[3].status,
        TaskStatus::Dropped
    );
    assert!(screen.leftovers().is_empty());
    assert_eq!(screen.leftover_selection(), None);
    assert_eq!(screen.selection(), Some(0), "back on today's rows");
}

#[test]
fn rhythm_drop_all_closes_the_block_and_undecided_leftovers_stay_open_on_their_day() {
    let store = store_with(&[october_first()]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    let screen = tab(persist(screen, &effects, &store, now), now);
    let (screen, effects) = screen.update(ScreenKey::Char('d'), now);
    let screen = persist(screen, &effects, &store, now);
    assert_eq!(titles(&screen), ["請求書の確認"]);
    drop(screen);
    let (reopened, _) = opened(&store, at(oct(2), 13, 0));
    assert_eq!(
        titles(&reopened),
        ["請求書の確認"],
        "undecided leftovers stay open and are offered again"
    );
    let reopened = tab(reopened, now);
    let (reopened, effects) = reopened.update(ScreenKey::Char('D'), now);
    persist(reopened.clone(), &effects, &store, now);
    assert!(reopened.leftovers().is_empty());
    assert_eq!(
        store.load(oct(1)).unwrap().tasks()[4].status,
        TaskStatus::Dropped
    );
}

#[test]
fn rhythm_block_navigation_and_task_keys_never_touch_todays_rows_from_the_block() {
    let mut today = Day::new(oct(2), Tuning::default());
    today = add(today, "今日の作業", TaskKind::Untimed, None);
    let store = store_with(&[october_first(), today]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    let screen = tab(persist(screen, &effects, &store, now), now);
    let (screen, effects) = screen.update(ScreenKey::Char(' '), now);
    assert!(effects.is_empty(), "Space in the block changes no task");
    let screen = screen.update(ScreenKey::Up, now).0;
    assert_eq!(screen.leftover_selection(), Some(0));
    let screen = screen.update(ScreenKey::Down, now).0;
    let screen = screen.update(ScreenKey::Down, now).0;
    assert_eq!(screen.leftover_selection(), None);
    assert_eq!(screen.selection(), Some(0));
    let (screen, effects) = screen.update(ScreenKey::Char('C'), now);
    assert!(effects.is_empty(), "C belongs to the block only");
    let screen = screen.update(ScreenKey::Up, now).0;
    assert_eq!(screen.leftover_selection(), Some(1));
    let (screen, effects) = screen.update(ScreenKey::Char('d'), now);
    assert_eq!(effects, vec![Effect::Save, Effect::SaveLeftovers]);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Open);
}

#[test]
fn rhythm_a_refused_carry_over_changes_neither_day() {
    let mut tuning = Tuning::default();
    tuning.day.tasks_per_day = 1;
    let full = Day::new(oct(2), tuning)
        .add(
            "満杯".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap()
        .0;
    let previous = october_first();
    assert_eq!(
        rhythm::decide_all(
            full.clone(),
            previous.clone(),
            LeftoverDecision::CarryOver,
            UnixMillis(5)
        ),
        Err(bunshin_core::day::DayError::LimitReached)
    );
    assert_eq!(
        rhythm::decide(
            full.clone(),
            previous.clone(),
            &[6],
            LeftoverDecision::Drop,
            UnixMillis(5)
        ),
        Err(bunshin_core::day::DayError::InvalidStatus),
        "an appointment is not a leftover"
    );
    assert_eq!(
        rhythm::decide(
            full.clone(),
            previous.clone(),
            &[99],
            LeftoverDecision::Drop,
            UnixMillis(5)
        ),
        Err(bunshin_core::day::DayError::TaskNotFound)
    );
    assert_eq!(
        rhythm::decide(full, previous, &[], LeftoverDecision::Drop, UnixMillis(5)),
        Err(bunshin_core::day::DayError::TaskNotFound)
    );
}

#[test]
fn rhythm_chat_carry_over_and_drop_proposals_share_one_change_set_with_the_earlier_day() {
    let store = store_with(&[october_first()]);
    let now = at(oct(2), 9, 2);
    let (screen, effects) = opened(&store, now);
    let mut screen = persist(screen, &effects, &store, now);
    for character in "経費精算は持ち越し、請求書はやめる".chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }
    let (screen, effects) = screen.update(ScreenKey::Enter, now);
    let screen = persist(screen, &effects, &store, now);
    let (screen, request, _) = screen.prepare_chat(&owner(), now);
    let request = request.expect("owner call");
    let prompt: serde_json::Value = serde_json::from_str(&request.request.prompt).unwrap();
    assert_eq!(prompt["leftovers"].as_array().unwrap().len(), 2);
    assert!(request.request.instructions.contains("carryOver"));
    let answer = ModelAnswer {
        json: r#"{"reply":"了解","changes":[{"op":"carryOver","task":4},{"op":"dropLeftover","task":5},{"op":"carryOver","task":6}]}"#.into(),
    };
    let (screen, effects) = screen.finish_chat(request.id, Ok(answer), now);
    assert!(effects.contains(&Effect::Save));
    assert!(effects.contains(&Effect::SaveLeftovers));
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, Effect::ChatNotice(_)))
            .count(),
        1,
        "the appointment is refused"
    );
    let screen = persist(screen, &effects, &store, now);
    assert_eq!(screen.day().tasks().len(), 1);
    assert_eq!(screen.day().tasks()[0].title, "経費精算");
    let earlier = store.load(oct(1)).unwrap();
    assert_eq!(earlier.tasks()[3].status, TaskStatus::CarriedOver);
    assert_eq!(earlier.tasks()[4].status, TaskStatus::Dropped);
    assert!(screen.leftovers().is_empty());
    let (screen, effects) = screen.update(ScreenKey::Undo, now);
    assert_eq!(effects, vec![Effect::Save, Effect::SaveLeftovers]);
    persist(screen, &effects, &store, now);
    assert_eq!(
        store
            .load(oct(1))
            .unwrap()
            .tasks()
            .iter()
            .filter(|task| task.status == TaskStatus::Open)
            .count(),
        3
    );
}

#[test]
fn rhythm_chat_without_leftovers_refuses_leftover_proposals() {
    let day = Day::new(oct(2), Tuning::default());
    let answer = ModelAnswer {
        json: r#"{"reply":"了解","changes":[{"op":"carryOver","task":1},{"op":"dropLeftover"}]}"#
            .into(),
    };
    let outcome =
        bunshin_core::prompt::answer::apply_chat(&day, &answer, UnixMillis(0), Tuning::default())
            .unwrap();
    assert_eq!(outcome.refused.len(), 2);
    assert_eq!(outcome.leftovers, None);
    assert_eq!(outcome.change_set, None);
}

#[test]
fn rhythm_evening_review_is_queued_once_at_eighteen_and_changes_no_task() {
    let store = store_with(&[]);
    let morning = at(oct(2), 9, 0);
    let (screen, effects) = opened(&store, morning);
    let screen = persist(screen, &effects, &store, morning);
    let (screen, _) = screen.take_checkin_batches();
    let screen = add_by_key(screen, morning);
    let tasks = screen.day().tasks().to_vec();
    let (screen, effects) = screen.check_evening_review(at(oct(2), 17, 59));
    assert!(effects.is_empty());
    let (screen, effects) = screen.check_evening_review(at(oct(2), 18, 0));
    assert_eq!(effects, vec![Effect::Save]);
    let (screen, batches) = screen.take_checkin_batches();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].reason, BatchReason::EveningReview);
    assert_eq!(batches[0].triggers[0].kind, TriggerKind::EveningReview);
    assert_eq!(screen.day().tasks(), tasks.as_slice());
    let (screen, effects) = screen.check_evening_review(at(oct(2), 18, 30));
    assert!(effects.is_empty(), "once a day");
    assert!(screen.take_checkin_batches().1.is_empty());
}

fn add_by_key(screen: MainScreen, now: Now) -> MainScreen {
    let mut screen = screen.update(ScreenKey::Tab, now).0;
    screen = screen.update(ScreenKey::Char('a'), now).0;
    for character in "メール".chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }
    screen.update(ScreenKey::Enter, now).0
}

#[test]
fn rhythm_evening_review_obeys_the_mute_and_waits_held() {
    let store = store_with(&[]);
    let morning = at(oct(2), 9, 0);
    let (screen, effects) = opened(&store, morning);
    let screen = tab(persist(screen, &effects, &store, morning), morning);
    let (screen, _) = screen.take_checkin_batches();
    let muting = at(oct(2), 17, 50);
    let (screen, _) = screen.update(ScreenKey::Char('m'), muting);
    let (screen, effects) = screen.check_evening_review(at(oct(2), 18, 0));
    assert_eq!(effects, vec![Effect::Save]);
    let (screen, batches) = screen.take_checkin_batches();
    assert!(batches.is_empty(), "the mute holds the review");
    assert!(
        screen
            .day()
            .data()
            .held_triggers
            .iter()
            .any(|trigger| trigger.kind == TriggerKind::EveningReview)
    );
}

#[test]
fn rhythm_opening_at_nineteen_thirty_starts_the_day_first_and_reviews_after_the_leftovers() {
    let store = store_with(&[october_first()]);
    let now = at(oct(2), 19, 30);
    let (screen, effects) = opened(&store, now);
    let screen = persist(screen, &effects, &store, now);
    let (screen, batches) = screen.take_checkin_batches();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].triggers[0].kind, TriggerKind::DayStart);
    let screen = tab(screen, now);
    let (screen, effects) = screen.check_evening_review(now);
    assert!(effects.is_empty(), "the leftovers are not settled yet");
    let settled = at(oct(2), 19, 31);
    let (screen, effects) = screen.update(ScreenKey::Char('D'), settled);
    assert_eq!(effects, vec![Effect::Save, Effect::SaveLeftovers]);
    let (_, batches) = screen.take_checkin_batches();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].triggers[0].kind, TriggerKind::EveningReview);
}

#[test]
fn rhythm_opening_after_eighteen_without_leftovers_queues_the_review_after_the_day_start() {
    let store = store_with(&[]);
    let now = at(oct(2), 19, 30);
    let (screen, _) = opened(&store, now);
    let (_, batches) = screen.take_checkin_batches();
    let kinds = batches
        .iter()
        .map(|batch| batch.triggers[0].kind)
        .collect::<Vec<_>>();
    assert_eq!(kinds, [TriggerKind::DayStart, TriggerKind::EveningReview]);
}

#[test]
fn rhythm_review_is_due_only_on_its_logical_day_after_the_day_start() {
    let tuning = Tuning::default();
    let day = Day::new(oct(2), tuning);
    assert!(!rhythm::review_due(&day, None, at(oct(2), 19, 0), tuning));
    let started = rhythm::start_day(
        day,
        None,
        Checkin::new(at(oct(2), 9, 0), tuning),
        at(oct(2), 9, 0),
        false,
        tuning,
    )
    .day;
    assert!(rhythm::review_due(
        &started,
        None,
        at(oct(2), 18, 0),
        tuning
    ));
    assert!(
        rhythm::review_due(&started, None, at(oct(3), 2, 0), tuning),
        "02:00 still belongs to 10/2"
    );
    assert!(!rhythm::review_due(
        &started,
        None,
        at(oct(3), 4, 0),
        tuning
    ));
    let mut early = tuning;
    early.rhythm.evening_review = time(2, 0, 0, 0);
    assert!(!rhythm::review_due(
        &started,
        None,
        at(oct(2), 23, 0),
        early
    ));
    assert!(rhythm::review_due(&started, None, at(oct(3), 2, 0), early));
    let update = rhythm::evening_review(
        started.clone(),
        None,
        Checkin::new(at(oct(2), 9, 0), tuning),
        at(oct(2), 17, 0),
        false,
        tuning,
    );
    assert_eq!(update.ready, None);
    assert!(!update.save);
    assert_eq!(update.day, started);
}

#[test]
fn rhythm_every_model_call_of_the_day_carries_yesterdays_record() {
    let store = store_with(&[october_first()]);
    let now = at(oct(2), 9, 2);
    let (screen, _) = opened(&store, now);
    let record = screen.day().data().yesterday_record.clone().unwrap().text;
    let chat = build_chat(
        screen.day(),
        &owner(),
        "おはよう",
        now,
        ContextExtras::default(),
        Tuning::default(),
    )
    .unwrap();
    let checkin = build_checkin(
        screen.day(),
        &owner(),
        now,
        &[],
        ContextExtras::default(),
        Tuning::default(),
    )
    .unwrap();
    for request in [chat.request, checkin.request] {
        let prompt: serde_json::Value = serde_json::from_str(&request.prompt).unwrap();
        assert_eq!(prompt["yesterday"], serde_json::json!(record));
    }
}

#[test]
fn rhythm_yesterdays_record_counts_each_status_with_at_most_three_titles() {
    let tuning = Tuning::default();
    let mut day = Day::new(oct(1), tuning);
    for number in 1..=5 {
        day = add(day, &format!("完了{number}"), TaskKind::Untimed, None);
        day = done(day, number);
    }
    day = add(day, "残り", TaskKind::Untimed, None);
    let record = day_record(&day, tuning);
    assert_eq!(record.done.count, 5);
    assert_eq!(record.done.titles, ["完了1", "完了2", "完了3"]);
    assert_eq!(record.open.count, 1);
    assert_eq!(record.dropped.count, 0);
    assert_eq!(record.carried_over.count, 0);
    assert_eq!(
        yesterday_text(&record, tuning),
        "2026-10-01の記録: 完了5件（完了1、完了2、完了3）、持ち越し0件、やめた0件、未完了1件（残り）"
    );
}

#[test]
fn rhythm_yesterdays_record_stays_within_its_token_bound_by_shortening_then_leaving_titles_out() {
    let tuning = Tuning::default();
    let mut day = Day::new(oct(1), tuning);
    for number in 1..=12 {
        day = add(day, &"長".repeat(80), TaskKind::Untimed, None);
        match number % 4 {
            0 => day = done(day, number),
            1 => day = day.drop(number, UnixMillis(0)).unwrap().0,
            2 => {
                let (next, _) = day
                    .settle_leftover(number, LeftoverDecision::CarryOver, UnixMillis(0))
                    .unwrap();
                day = next;
            }
            _ => {}
        }
    }
    let text = yesterday_text(&day_record(&day, tuning), tuning);
    assert!(estimate(&text, 2) <= 120, "{}", estimate(&text, 2));
    assert!(text.contains("完了3件"));
    assert!(text.contains("持ち越し3件"));
    assert!(text.contains("やめた3件"));
    assert!(text.contains("未完了3件（長長長長長長長…"));
    let mut tiny = tuning;
    tiny.prompt.yesterday_tokens = 10;
    let text = yesterday_text(&day_record(&day, tuning), tiny);
    assert!(estimate(&text, 2) <= 10);
    assert!(text.starts_with("2026-10-01"));
}

#[test]
fn rhythm_leftover_facts_round_trip_in_the_day_file_and_older_rows_omit_them() {
    let previous = october_first();
    let decided = rhythm::decide_all(
        Day::new(oct(2), Tuning::default()),
        previous,
        LeftoverDecision::CarryOver,
        UnixMillis(7),
    )
    .unwrap();
    let value = serde_json::to_value(DayFile::from(&decided.day)).unwrap();
    let row = &value["messages"][0]["changeSet"];
    assert_eq!(row["leftovers"][0]["date"], "2026-10-01");
    assert_eq!(row["leftovers"][0]["after"]["status"], "carriedOver");
    let reread = serde_json::from_value::<DayFile>(value)
        .unwrap()
        .into_day(Tuning::default())
        .unwrap();
    assert_eq!(
        reread.messages()[0].change_set,
        Some(decided.change.clone())
    );
    let plain = add(
        Day::new(oct(2), Tuning::default()),
        "a",
        TaskKind::Untimed,
        None,
    );
    let value = serde_json::to_value(DayFile::from(&plain)).unwrap();
    assert!(value["messages"][0]["changeSet"].get("leftovers").is_none());
    let mut corrupt = serde_json::to_value(DayFile::from(&decided.day)).unwrap();
    corrupt["messages"][0]["changeSet"]["leftovers"][0]["after"]["closedAt"] =
        serde_json::Value::Null;
    assert!(matches!(
        serde_json::from_value::<DayFile>(corrupt)
            .unwrap()
            .into_day(Tuning::default()),
        Err(bunshin_core::day::file::DayFileError::InvalidTask { .. })
    ));
}

#[test]
fn rhythm_undo_reopens_only_the_leftovers_it_decided_on_their_own_unchanged_day() {
    let decided = rhythm::decide_all(
        Day::new(oct(2), Tuning::default()),
        october_first(),
        LeftoverDecision::Drop,
        UnixMillis(7),
    )
    .unwrap();
    let other = Day::new(date(2026, 9, 30), Tuning::default());
    let undone = rhythm::undo(decided.day.clone(), Some(other.clone()), UnixMillis(8)).unwrap();
    assert!(!undone.previous_changed, "another day is never touched");
    assert_eq!(undone.previous, Some(other));
    let (reopened, _) = decided.previous.clone().reopen(4, UnixMillis(9)).unwrap();
    let undone = rhythm::undo(decided.day.clone(), Some(reopened), UnixMillis(10)).unwrap();
    assert!(undone.previous_changed, "the untouched leftover reopens");
    let statuses = undone
        .previous
        .unwrap()
        .tasks()
        .iter()
        .map(|task| task.status)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses[3..],
        [TaskStatus::Open, TaskStatus::Open, TaskStatus::Open]
    );
    let undone = rhythm::undo(decided.day, None, UnixMillis(11)).unwrap();
    assert!(!undone.previous_changed);
    assert_eq!(undone.change.leftovers.len(), 2);
}
