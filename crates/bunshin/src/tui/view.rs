//! Draw one pure screen value without reading the terminal or changing state.
mod overlays;
#[cfg(test)]
#[path = "view/snapshots.rs"]
mod snapshots;
#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;

use crate::wording;
use bunshin_core::{
    Now,
    day::{MessageKind, TaskStatus},
    screen::{
        Focus, MainScreen, SaveState, ScreenError,
        help::{help_rows, task_help},
        keys::{KeyBinding, KeyRegion},
    },
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, List, ListItem, ListState, Paragraph, Wrap},
};

const BASE_STYLE: Style = Style::new().fg(Color::Reset).bg(Color::Reset);
const FOCUSED_STYLE: Style = BASE_STYLE.fg(Color::Green).add_modifier(Modifier::BOLD);
const SELECTED_FOCUSED: Style = BASE_STYLE.add_modifier(Modifier::REVERSED);
const SELECTED_UNFOCUSED: Style = BASE_STYLE.add_modifier(Modifier::BOLD);
const DROPPED_STYLE: Style = BASE_STYLE.add_modifier(Modifier::CROSSED_OUT);
const ERROR_STYLE: Style = BASE_STYLE.fg(Color::Red);
const ERROR_LABEL_STYLE: Style = ERROR_STYLE.add_modifier(Modifier::BOLD);
const KEY_STYLE: Style = BASE_STYLE.add_modifier(Modifier::BOLD);

pub fn draw(frame: &mut Frame, screen: &MainScreen, now: Now) {
    let area = frame.area();
    if area.width < 60 || area.height < 18 {
        let message = Rect::new(
            area.x,
            area.y + area.height.saturating_div(2).saturating_sub(1),
            area.width,
            area.height.min(2),
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(wording::TOO_SMALL),
                Line::from(wording::EXPAND_TERMINAL),
            ])
            .alignment(Alignment::Center),
            message,
        );
        return;
    }
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(frame, screen, now, header);
    let (tasks, right) = if area.width >= 100 {
        let [tasks, right] =
            Layout::horizontal([Constraint::Length(36), Constraint::Min(0)]).areas(body);
        (tasks, right)
    } else {
        let rows = u16::try_from(screen.day().tasks().len())
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .clamp(3, 12)
            .min(body.height.saturating_sub(6));
        let [tasks, right] =
            Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).areas(body);
        (tasks, right)
    };
    draw_tasks(frame, screen, tasks);
    let [chat, input] = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(right);
    draw_errors(frame, screen, chat);
    frame.render_widget(
        region_block(wording::INPUT_TITLE, screen.focus() == Focus::Input),
        input,
    );
    if screen.is_confirming_quit() {
        let confirmation = match screen.save_state() {
            SaveState::Saved | SaveState::NotSaved(_) => wording::QUIT_UNSAVED,
            SaveState::DurabilityUnconfirmed => wording::QUIT_UNCONFIRMED,
        };
        frame.render_widget(
            Paragraph::new(confirmation).style(ERROR_LABEL_STYLE),
            footer,
        );
    } else {
        frame.render_widget(
            Paragraph::new(binding_line(&footer_bindings(screen.focus()))),
            footer,
        );
    }
    overlays::draw(frame, screen);
}
fn draw_header(frame: &mut Frame, screen: &MainScreen, now: Now, area: Rect) {
    let mut spans = vec![
        Span::styled(format!(" {}  ", wording::APP_NAME), KEY_STYLE),
        Span::raw(wording::date_and_clock(screen.day(), now)),
    ];
    match screen.save_state() {
        SaveState::Saved => {}
        SaveState::NotSaved(_) => spans.push(Span::styled(
            format!("  {}", wording::NOT_SAVED),
            ERROR_LABEL_STYLE,
        )),
        SaveState::DurabilityUnconfirmed => spans.push(Span::styled(
            format!("  {}", wording::NOT_DURABLE),
            ERROR_LABEL_STYLE,
        )),
    }
    let [left, right] = Layout::horizontal([Constraint::Min(0), Constraint::Length(6)]).areas(area);
    frame.render_widget(Paragraph::new(Line::from(spans)), left);
    frame.render_widget(
        Paragraph::new(wording::WAITING).alignment(Alignment::Right),
        right,
    );
}
fn region_block(title: &str, focused: bool) -> Block<'_> {
    Block::bordered()
        .title(title)
        .border_type(if focused {
            BorderType::Thick
        } else {
            BorderType::Plain
        })
        .border_style(if focused { FOCUSED_STYLE } else { BASE_STYLE })
        .title_style(if focused { FOCUSED_STYLE } else { BASE_STYLE })
}
fn draw_tasks(frame: &mut Frame, screen: &MainScreen, area: Rect) {
    let focused = screen.focus() == Focus::Tasks;
    let block = region_block(wording::TASKS_TITLE, focused).title_top(
        Line::from(wording::task_count(
            screen.day().open_task_count(),
            screen.day().tasks().len(),
        ))
        .right_aligned(),
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let tasks = screen.day().task_view();
    if tasks.is_empty() {
        frame.render_widget(
            Paragraph::new(wording::EMPTY_TASKS).wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }
    let rows = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let marker = if screen.selection() == Some(index) {
                '>'
            } else {
                ' '
            };
            let time = wording::task_time(task);
            let time_width = Span::raw(&time).width();
            let prefix = format!(
                "{marker}{} {} {time}{} ",
                task.number,
                wording::status_mark(task.status),
                " ".repeat(7_usize.saturating_sub(time_width))
            );
            let prefix_width = Span::raw(&prefix).width();
            let title = truncate(
                &task.title,
                usize::from(inner.width).saturating_sub(prefix_width),
            );
            let style = if task.status == TaskStatus::Dropped {
                DROPPED_STYLE
            } else {
                BASE_STYLE
            };
            ListItem::new(format!("{prefix}{title}")).style(style)
        })
        .collect::<Vec<_>>();
    let mut selection = ListState::default().with_selected(screen.selection());
    frame.render_stateful_widget(
        List::new(rows).highlight_style(if focused {
            SELECTED_FOCUSED
        } else {
            SELECTED_UNFOCUSED
        }),
        inner,
        &mut selection,
    );
}
fn truncate(text: &str, width: usize) -> String {
    let text: String = text
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let span = Span::raw(&text);
    if span.width() <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut used = 0;
    for grapheme in span.styled_graphemes(BASE_STYLE) {
        let columns = Span::raw(grapheme.symbol).width();
        if used + columns > width - 1 {
            break;
        }
        result.push_str(grapheme.symbol);
        used += columns;
    }
    result.push('…');
    result
}
fn draw_errors(frame: &mut Frame, screen: &MainScreen, area: Rect) {
    let block = region_block(wording::CHAT_TITLE, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut rows = screen
        .day()
        .messages()
        .iter()
        .rev()
        .find(|message| message.kind == MessageKind::Error)
        .into_iter()
        .map(|message| {
            Line::from(vec![
                Span::styled(format!("{}  ", wording::ERROR_SPEAKER), ERROR_LABEL_STYLE),
                Span::styled(&message.text, ERROR_STYLE),
            ])
        })
        .collect::<Vec<_>>();
    if let Some(ScreenError::Day(error)) = screen.error() {
        rows.push(Line::from(vec![
            Span::styled(format!("{}  ", wording::ERROR_SPEAKER), ERROR_LABEL_STYLE),
            Span::styled(wording::day_error(error), ERROR_STYLE),
        ]));
    }
    // Full chat history arrives with the chat use case. Keep the latest error visible now.
    frame.render_widget(Paragraph::new(rows).wrap(Wrap { trim: false }), inner);
}
fn footer_bindings(focus: Focus) -> Vec<&'static KeyBinding> {
    if focus == Focus::Tasks {
        return task_help();
    }
    let region = match focus {
        Focus::Input => KeyRegion::Main,
        Focus::Tasks => KeyRegion::Tasks,
        Focus::Form => KeyRegion::Form,
        Focus::Help => KeyRegion::Help,
    };
    help_rows()
        .iter()
        .filter(|binding| binding.region == region || binding.region == KeyRegion::Anywhere)
        .collect()
}
fn binding_line(bindings: &[&KeyBinding]) -> Line<'static> {
    let mut spans = Vec::new();
    for binding in bindings {
        let keys = binding
            .keys
            .iter()
            .map(|key| wording::key_label(*key))
            .collect::<Vec<_>>()
            .join("/");
        spans.push(Span::styled(keys, KEY_STYLE));
        spans.push(Span::raw(format!(
            " {}  ",
            wording::action_label(binding.action)
        )));
    }
    Line::from(spans)
}
