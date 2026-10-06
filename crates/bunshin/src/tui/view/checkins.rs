//! Check-ins on screen: tag lines, the bell, header items, T4, and the leftovers block.
use super::tests::render;
use super::*;
use crate::tui::controller::process_effects;
use bunshin_core::{
    Availability, ModelAnswer, Tuning, UnixMillis,
    day::{
        Author, Day, InboxState, Message, MessageKind, TaskKind, TaskOrigin, Trigger, TriggerKind,
        UnpromptedKind, UnpromptedMessage,
        file::{DayData, DayFile},
        store::DayStore,
    },
    instructions::InstructionsState,
    screen::ScreenKey,
};
use bunshin_test_support::InMemoryDayStore;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};

const BASE: i64 = 1_790_000_000_000;

/// 2026-10-02 (a Friday) at the given civil time, with instants on one fixed offset.
fn at(hour: i64, minute: i64) -> Now {
    Now {
        instant: UnixMillis(BASE + (hour * 3_600 + minute * 60) * 1_000),
        local: format!("2026-10-02T{hour:02}:{minute:02}:00")
            .parse()
            .unwrap(),
    }
}
fn text(buffer: &Buffer) -> Vec<String> {
    (0..buffer.area.height).map(|y| row(buffer, y)).collect()
}
fn row(buffer: &Buffer, y: u16) -> String {
    let mut text = String::new();
    let mut x = 0;
    while x < buffer.area.width {
        let symbol = buffer[(x, y)].symbol();
        text.push_str(symbol);
        x += u16::try_from(Span::raw(symbol).width().max(1)).unwrap();
    }
    text
}
/// The column of the first cell whose symbol is `symbol` on row `y`.
fn column(buffer: &Buffer, y: u16, symbol: &str) -> u16 {
    (0..buffer.area.width)
        .find(|x| buffer[(*x, y)].symbol() == symbol)
        .unwrap_or_else(|| panic!("{symbol} on row {y}: {}", row(buffer, y)))
}
fn row_with(buffer: &Buffer, needle: &str) -> u16 {
    (0..buffer.area.height)
        .find(|y| row(buffer, *y).contains(needle))
        .unwrap_or_else(|| panic!("{needle} missing:\n{}", text(buffer).join("\n")))
}
fn day_with(edit: impl FnOnce(&mut DayData)) -> Day {
    let mut data = Day::new("2026-10-02".parse().unwrap(), Tuning::default())
        .data()
        .clone();
    edit(&mut data);
    (DayFile { format: 1, data })
        .into_day(Tuning::default())
        .unwrap()
}
fn unprompted(
    kind: UnpromptedKind,
    trigger: TriggerKind,
    task: Option<u64>,
    text: &str,
    time: UnixMillis,
) -> Message {
    Message {
        author: Author::Bunshin,
        text: text.into(),
        time,
        kind: MessageKind::Unprompted,
        answers_question: None,
        change_set: None,
        cancelled: false,
        in_reply_to: None,
        unprompted: Some(UnpromptedMessage {
            kind,
            trigger: Trigger {
                kind: trigger,
                task,
                due_at: time,
            },
            task,
            inbox_state: InboxState::Open,
            state_changed_at: time,
            suppressed: None,
        }),
    }
}
fn owner() -> InstructionsState {
    InstructionsState::resolve(Some("秘書"), "instructions.md".into(), Tuning::default())
}
fn render_with_layout(screen: MainScreen, now: Now, width: u16, height: u16) -> MainScreen {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut metrics = (0, 0, None);
    terminal
        .draw(|frame| {
            metrics = draw_with_metrics(frame, &screen, now, &|at| now.at_fixed_offset(at));
        })
        .unwrap();
    screen.record_chat_layout(metrics.0, metrics.1)
}

#[test]
fn a_check_in_question_rings_once_and_opens_with_a_magenta_bold_tag_row() {
    let start = at(14, 0);
    let store = InMemoryDayStore::new(Tuning::default());
    let day = Day::new("2026-10-02".parse().unwrap(), Tuning::default())
        .add(
            "資料作成".into(),
            TaskKind::Deadline,
            Some("15:00".parse().unwrap()),
            TaskOrigin::Key,
            start.instant,
        )
        .unwrap()
        .0;
    let mut bells = 0;
    let (screen, effects) = MainScreen::new(day, Tuning::default()).open_checkins(start);
    let screen = process_effects(screen, effects, start, &store, || {}, &mut bells);
    let screen = screen
        .record_availability(Ok(Availability::Available), start.instant)
        .0;
    let (screen, request, _) = screen.prepare_checkin(&owner(), start, wording::fixed_deadline);
    let (screen, effects) = screen.finish_checkin(
        request.unwrap().id,
        Ok(ModelAnswer {
            json: r#"{"kind":"silent","message":"","next_look_minutes":120}"#.into(),
        }),
        start,
        wording::fixed_deadline,
    );
    let mut screen = process_effects(screen, effects, start, &store, || {}, &mut bells);
    assert_eq!(bells, 0);
    let mut request = None;
    for minute in 1..=30 {
        let now = at(14, minute);
        let (next, effects) = screen.tick(now);
        screen = process_effects(next, effects, now, &store, || {}, &mut bells);
        let (next, prepared, effects) =
            screen.prepare_checkin(&owner(), now, wording::fixed_deadline);
        screen = process_effects(next, effects, now, &store, || {}, &mut bells);
        if prepared.is_some() {
            assert_eq!(minute, 30, "the 30-minute notice fires at 14:30");
            request = prepared;
        }
    }
    let now = at(14, 30);
    let (screen, effects) = screen.finish_checkin(
        request.unwrap().id,
        Ok(ModelAnswer {
            json: r#"{"kind":"question","task":1,"message":"資料、どこまで進んだ？","next_look_minutes":30}"#
                .into(),
        }),
        now,
        wording::fixed_deadline,
    );
    let screen = process_effects(screen, effects, now, &store, || {}, &mut bells);
    assert_eq!(bells, 1);
    let mut out = Vec::new();
    crate::tui::controller::ring(&mut out, bells).unwrap();
    assert_eq!(out, b"\x07");
    let buffer = render(&screen, now, 120, 30);
    let tag = row_with(&buffer, "14:30 Bunshin [質問 / 締切30分前: 資料作成]");
    assert!(row(&buffer, tag + 1).contains("資料、どこまで進んだ？"));
    let bracket = column(&buffer, tag, "[");
    for x in [bracket, bracket + 1] {
        assert_eq!(buffer[(x, tag)].fg, Color::Magenta);
        assert!(buffer[(x, tag)].modifier.contains(Modifier::BOLD));
    }
    assert!(row(&buffer, 0).contains("受信箱 1 | 次の見回り 15:00 | 待機中"));
    let inbox = column(&buffer, 0, "受");
    assert_eq!(buffer[(inbox, 0)].fg, Color::Magenta);
    assert!(buffer[(inbox, 0)].modifier.contains(Modifier::BOLD));
}

#[test]
fn a_note_tag_is_magenta_without_bold_and_the_new_divider_is_magenta() {
    let now = at(13, 10);
    let note = day_with(|data| {
        data.messages.push(unprompted(
            UnpromptedKind::Note,
            TriggerKind::PlannedLook,
            None,
            "午後はメール返信から片付けると楽そう",
            now.instant,
        ));
    });
    let buffer = render(&MainScreen::new(note, Tuning::default()), now, 100, 24);
    let tag = row_with(&buffer, "13:10 Bunshin [お知らせ / 予定した見回り]");
    let bracket = column(&buffer, tag, "[");
    assert_eq!(buffer[(bracket, tag)].fg, Color::Magenta);
    assert!(!buffer[(bracket, tag)].modifier.contains(Modifier::BOLD));
    assert!(row(&buffer, tag + 1).contains("午後はメール返信から片付けると楽そう"));
    let fresh = MainScreen::new(
        Day::new("2026-10-02".parse().unwrap(), Tuning::default()),
        Tuning::default(),
    )
    .record_chat_notice(
        bunshin_core::screen::ChatNotice::ModelBack,
        "synthetic",
        now.instant,
    );
    let buffer = render(&fresh, now, 100, 24);
    let divider = row_with(&buffer, "── ここから新着 ──");
    let dash = column(&buffer, divider, "─");
    assert_eq!(buffer[(dash, divider)].fg, Color::Magenta);
    assert!(!buffer[(dash, divider)].modifier.contains(Modifier::BOLD));
}

#[test]
fn header_items_match_the_eighty_column_variants_and_their_roles() {
    let question = |data: &mut DayData| {
        data.messages.push(unprompted(
            UnpromptedKind::Question,
            TriggerKind::PlannedLook,
            None,
            "synthetic question",
            at(14, 30).instant,
        ));
        data.next_planned_look = Some("2026-10-02T15:00:00".parse().unwrap());
    };
    let now = at(14, 31);
    let normal = MainScreen::new(day_with(question), Tuning::default());
    let held = MainScreen::new(
        day_with(|data| {
            question(data);
            data.held_triggers.push(Trigger {
                kind: TriggerKind::PlannedLook,
                task: None,
                due_at: now.instant,
            });
        }),
        Tuning::default(),
    )
    .update(ScreenKey::Char('a'), now)
    .0;
    let muted = MainScreen::new(
        day_with(|data| {
            data.next_planned_look = Some("2026-10-02T15:00:00".parse().unwrap());
            data.muted_until = Some(UnixMillis(now.instant.0 + 3_600_000));
        }),
        Tuning::default(),
    );
    let late = MainScreen::new(
        day_with(|data| data.next_planned_look = Some("2026-10-03T08:00:00".parse().unwrap())),
        Tuning::default(),
    );
    for (screen, now, expected) in [
        (
            &normal,
            now,
            " Bunshin  10/2(金) 14:31                    受信箱 1 | 次の見回り 15:00 | 待機中",
        ),
        (
            &held,
            now,
            " Bunshin  10/2(金) 14:31           受信箱 1 | 保留 1 | 次の見回り 15:00 | 待機中",
        ),
        (
            &muted,
            now,
            " Bunshin  10/2(金) 14:31          ミュート中 〜15:31 | 次の見回り 15:00 | 待機中",
        ),
        (
            &late,
            at(23, 10),
            " Bunshin  10/2(金) 23:10        時間外（08:00 から） | 次の見回り 08:00 | 待機中",
        ),
    ] {
        let buffer = render(screen, now, 80, 24);
        assert_eq!(row(&buffer, 0), expected);
    }
    let buffer = render(&held, now, 80, 24);
    let held_label = column(&buffer, 0, "保");
    assert_eq!(buffer[(held_label, 0)].fg, Color::Reset);
    assert!(buffer[(held_label, 0)].modifier.contains(Modifier::BOLD));
    let separator = column(&buffer, 0, "|");
    assert_eq!(buffer[(separator, 0)].modifier, Modifier::empty());
    let buffer = render(&muted, now, 80, 24);
    let mute = column(&buffer, 0, "ミ");
    assert_eq!(buffer[(mute, 0)].fg, Color::Reset);
    assert!(buffer[(mute, 0)].modifier.contains(Modifier::BOLD));
    let buffer = render(&late, at(23, 10), 80, 24);
    let hours = column(&buffer, 0, "時");
    assert!(buffer[(hours, 0)].modifier.contains(Modifier::BOLD));
    let look = column(&buffer, 0, "次");
    assert_eq!(buffer[(look, 0)].modifier, Modifier::empty());
    // At the smallest width the date and the model state stay; the next look goes first.
    let buffer = render(&late, at(23, 10), 60, 18);
    assert_eq!(
        row(&buffer, 0),
        " Bunshin  10/2(金) 23:10       時間外（08:00 から） | 待機中"
    );
}

#[test]
fn a_key_mute_shows_its_change_line_and_the_header_item() {
    let now = at(15, 31);
    let screen = MainScreen::new(
        Day::new("2026-10-02".parse().unwrap(), Tuning::default()),
        Tuning::default(),
    )
    .update(ScreenKey::Tab, now)
    .0
    .update(ScreenKey::Char('m'), now)
    .0;
    let buffer = render(&screen, now, 80, 24);
    assert!(row(&buffer, 0).contains("ミュート中 〜16:31"));
    row_with(&buffer, "15:31 変更    ミュート 〜16:31（m で解除）");
}

#[test]
fn a_note_arriving_while_scrolled_up_keeps_the_view_and_counts_on_the_border() {
    let now = at(9, 0);
    let store = InMemoryDayStore::new(Tuning::default());
    let mut bells = 0;
    let (screen, effects) = MainScreen::new(
        Day::new("2026-10-02".parse().unwrap(), Tuning::default()),
        Tuning::default(),
    )
    .open_checkins(now);
    let mut screen = process_effects(screen, effects, now, &store, || {}, &mut bells)
        .record_availability(Ok(Availability::Available), now.instant)
        .0;
    for number in 0..30 {
        screen = screen.record_chat_notice(
            bunshin_core::screen::ChatNotice::ModelBack,
            &format!("synthetic row {number}"),
            now.instant,
        );
    }
    let (screen, request, _) = screen.prepare_checkin(&owner(), now, wording::fixed_deadline);
    let screen = render_with_layout(screen, now, 100, 24);
    let screen = screen.update(ScreenKey::PageUp, now).0;
    let screen = render_with_layout(screen, now, 100, 24);
    let top = screen.chat_scroll_top();
    let before = render(&screen, now, 100, 24);
    let (screen, effects) = screen.finish_checkin(
        request.unwrap().id,
        Ok(ModelAnswer {
            json: r#"{"kind":"note","message":"synthetic note","next_look_minutes":60}"#.into(),
        }),
        now,
        wording::fixed_deadline,
    );
    let screen = process_effects(screen, effects, now, &store, || {}, &mut bells);
    assert_eq!(bells, 1);
    let screen = render_with_layout(screen, now, 100, 24);
    assert_eq!(screen.chat_scroll_top(), top);
    let after = render(&screen, now, 100, 24);
    let border = row_with(&after, "新着 1（End で最新へ）");
    assert!(row(&before, border).starts_with("│") || !row(&before, border).contains("新着"));
    for y in 2..border {
        assert_eq!(row(&after, y), row(&before, y), "row {y} moved");
    }
}

#[test]
fn the_inbox_lists_open_items_newest_first_and_shows_its_empty_state() {
    let now = at(14, 31);
    for (width, height) in [(80, 24), (120, 30)] {
        let empty = MainScreen::new(
            Day::new("2026-10-02".parse().unwrap(), Tuning::default()),
            Tuning::default(),
        )
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::Char('b'), now)
        .0;
        let buffer = render(&empty, now, width, height);
        let title = row_with(&buffer, "┏ 受信箱");
        assert!(row(&buffer, title).contains("未対応 0"));
        row_with(&buffer, "対応が必要なものはありません。");
        let keys = row_with(&buffer, "Esc 戻る");
        assert!(!row(&buffer, keys).contains("返事する"));
        let day = day_with(|data| {
            data.messages.push(unprompted(
                UnpromptedKind::Note,
                TriggerKind::PlannedLook,
                None,
                "午後はメール返信から片付けると楽そう",
                at(13, 10).instant,
            ));
            data.messages.push(unprompted(
                UnpromptedKind::Question,
                TriggerKind::PlannedLook,
                None,
                "資料、どこまで進んだ？",
                at(14, 30).instant,
            ));
        });
        let open = MainScreen::new(day, Tuning::default())
            .update(ScreenKey::Tab, now)
            .0
            .update(ScreenKey::Char('b'), now)
            .0;
        let buffer = render(&open, now, width, height);
        let title = row_with(&buffer, "┏ 受信箱");
        assert!(row(&buffer, title).contains("未対応 2"));
        let first = row_with(&buffer, "> 14:30 [質問 / 予定した見回り]");
        assert!(row(&buffer, first + 1).contains("資料、どこまで進んだ？"));
        assert!(row(&buffer, first + 2).contains("  13:10 [お知らせ / 予定した見回り]"));
        assert!(row(&buffer, first + 3).contains("午後はメール返信から片付けると楽そう"));
        let marker = column(&buffer, first, ">");
        assert!(
            buffer[(marker, first)]
                .modifier
                .contains(Modifier::REVERSED)
        );
        let tag = column(&buffer, first, "[");
        assert_eq!(buffer[(tag, first)].fg, Color::Magenta);
        assert!(buffer[(tag, first)].modifier.contains(Modifier::BOLD));
        row_with(&buffer, "15分反応がないと「無視」として秘書に伝わります");
        row_with(
            &buffer,
            "Enter 返事する／了解  x 閉じる  X お知らせを全部了解  g タスクへ  Esc 戻る",
        );
        let answering = open.update(ScreenKey::Enter, now).0;
        assert_eq!(answering.inbox(), None);
        let buffer = render(&answering, now, width, height);
        row_with(&buffer, "入力（14:30 の質問への返事）");
    }
}

#[test]
fn the_leftovers_block_sits_above_today_with_its_own_key_line() {
    let now = at(9, 2);
    let tuning = Tuning::default();
    let previous = Day::new("2026-10-01".parse().unwrap(), tuning);
    let previous = previous
        .add(
            "経費精算".into(),
            TaskKind::Deadline,
            Some("17:00".parse().unwrap()),
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap()
        .0
        .add(
            "請求書の確認".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap()
        .0;
    let store = InMemoryDayStore::new(tuning);
    store.save(&previous).unwrap();
    let mut bells = 0;
    let (screen, effects) =
        MainScreen::new(Day::new("2026-10-02".parse().unwrap(), tuning), tuning).open_checkins(now);
    let screen = process_effects(screen, effects, now, &store, || {}, &mut bells)
        .update(ScreenKey::Tab, now)
        .0;
    assert_eq!(bells, 0);
    let buffer = render(&screen, now, 80, 24);
    let lines = text(&buffer);
    assert_eq!(
        lines[2],
        "┃  前日の残り 10/1(木) ─ 持ち越すか、やめるか決めてください                    ┃"
    );
    assert_eq!(
        lines[3],
        "┃> [ ] 〜17:00 経費精算                                                        ┃"
    );
    assert_eq!(
        lines[4],
        "┃  [ ]         請求書の確認                                                    ┃"
    );
    assert!(lines[5].starts_with("┃  ──"));
    assert_eq!(
        lines[6],
        "┃  （今日のタスクはまだありません）                                            ┃"
    );
    assert!(lines[23].starts_with("? 全キー  c 持ち越し  d やめる  C 全部持ち越し  D 全部やめる"));
    assert!(buffer[(3, 2)].modifier.contains(Modifier::BOLD));
    assert!(buffer[(1, 3)].modifier.contains(Modifier::REVERSED));
    let mut bells = 0;
    let screen = crate::tui::controller::process_key(
        screen,
        ScreenKey::Char('c'),
        now,
        &store,
        || {},
        &mut bells,
    );
    let buffer = render(&screen, now, 80, 24);
    let lines = text(&buffer);
    assert!(lines[3].starts_with("┃> [ ]         請求書の確認"));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("1 [ ]         経費精算"))
    );
}
