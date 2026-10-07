//! Hand-reviewed empty-screen frames; style roles have separate exact cell assertions.
use super::tests::{empty, render};
use ratatui::{
    backend::{Backend, TestBackend},
    style::Style,
};

fn assert_lines(width: u16, height: u16, lines: &[&str]) {
    let (screen, now) = empty();
    let mut buffer = render(&screen, now, width, height);
    buffer.set_style(buffer.area, Style::reset());
    let mut backend = TestBackend::new(width, height);
    backend
        .draw(buffer.content.iter().enumerate().map(|(index, cell)| {
            let position = u16::try_from(index).expect("small frame");
            (position % width, position / width, cell)
        }))
        .expect("test backend");
    backend.assert_buffer_lines(lines.iter().copied());
}
#[test]
fn wide_empty_frame_matches_the_full_120_by_30_snapshot() {
    assert_lines(
        120,
        30,
        &[
            " Bunshin  11/14(火) 22:13                                                   時間外（08:00 から） | 次の見回り — | 待機中",
            "┌ 今日のタスク ──────── 未完了 0/0 ┐┌ チャット ────────────────────────────────────────────────────────────────────────┐",
            "│まだありません。Tab で移って a    ││                                                                                  │",
            "│で追加                            ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  ││                                                                                  │",
            "│                                  │└──────────────────────────────────────────────────────────────────────────────────┘",
            "│                                  │┏ 入力 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
            "│                                  │┃                                                                                  ┃",
            "└──────────────────────────────────┘┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 0/400 ┛",
            "Enter 送信  Esc 中止／クリア  Ctrl+C 終了  Ctrl+Z 取り消し  Tab/Shift+Tab 入力欄⇔タスク欄  PgUp 前頁  PgDn 次頁  End 最 ",
        ],
    );
}
#[test]
fn narrow_empty_frame_matches_the_full_80_by_24_snapshot() {
    assert_lines(
        80,
        24,
        &[
            " Bunshin  11/14(火) 22:13           時間外（08:00 から） | 次の見回り — | 待機中",
            "┌ 今日のタスク ──────────────────────────────────────────────────── 未完了 0/0 ┐",
            "│まだありません。Tab で移って a で追加                                         │",
            "└──────────────────────────────────────────────────────────────────────────────┘",
            "┌ チャット ────────────────────────────────────────────────────────────────────┐",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "│                                                                              │",
            "└──────────────────────────────────────────────────────────────────────────────┘",
            "┏ 入力 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
            "┃                                                                              ┃",
            "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 0/400 ┛",
            "Enter 送信  Esc 中止／クリア  Ctrl+C 終了  Ctrl+Z 取り消し  Tab/Shift+Tab 入力欄",
        ],
    );
}
#[test]
fn small_frame_matches_the_full_59_by_17_snapshot() {
    assert_lines(
        59,
        17,
        &[
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                    端末が小さすぎます                     ",
            "                60×18 以上に広げてください                 ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
            "                                                           ",
        ],
    );
}
