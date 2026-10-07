//! Draw the task list and conversation without reading input or changing state.
#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;

use crate::wording;
use bunshin_core::{
    Now,
    day::{Author, MessageKind, TaskStatus},
    screen::{ChatStatus, Focus, MainScreen, SaveState, help::task_help, keys::KeyBinding},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

const BASE: Style = Style::new();
const FOCUSED: Style = BASE.fg(Color::Green).add_modifier(Modifier::BOLD);
const ERROR: Style = BASE.fg(Color::Red);
const MODEL: Style = BASE.fg(Color::Magenta);
const SYSTEM: Style = BASE.fg(Color::Blue);

pub(super) fn draw_with_metrics(
    frame: &mut Frame,
    screen: &MainScreen,
    now: Now,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
) -> (usize, usize) {
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
        return (0, 0);
    }
    let [header, body, footer_area] = Layout::vertical([
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
        let rows = u16::try_from(screen.day().tasks().len().max(1))
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .clamp(3, 12)
            .min(body.height.saturating_sub(6));
        let [tasks, right] =
            Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).areas(body);
        (tasks, right)
    };
    draw_tasks(frame, screen, tasks);
    let input_rows = screen.input().text().lines().count().clamp(1, 3);
    let input_height = u16::try_from(input_rows).unwrap_or(3).saturating_add(2);
    let [chat, input] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(input_height)]).areas(right);
    let metrics = draw_chat(frame, screen, now, chat, local_at);
    draw_input(frame, screen, input);
    let footer = if screen.is_confirming_quit() {
        match screen.save_state() {
            SaveState::Saved | SaveState::NotSaved(_) => wording::QUIT_UNSAVED,
            SaveState::DurabilityUnconfirmed => wording::QUIT_UNCONFIRMED,
        }
    } else {
        "Tab  画面切替    ?  キー操作    q  終了"
    };
    frame.render_widget(Paragraph::new(footer), footer_area);
    if screen.focus() == Focus::Help {
        draw_help(frame);
    }
    metrics
}

fn block(title: &str, focused: bool) -> Block<'_> {
    Block::bordered()
        .title(title)
        .border_type(if focused {
            BorderType::Thick
        } else {
            BorderType::Plain
        })
        .border_style(if focused { FOCUSED } else { BASE })
        .title_style(if focused { FOCUSED } else { BASE })
}

fn draw_header(frame: &mut Frame, screen: &MainScreen, now: Now, area: Rect) {
    let left = Line::from(vec![
        Span::styled(
            format!(" {}  ", wording::APP_NAME),
            BASE.add_modifier(Modifier::BOLD),
        ),
        Span::raw(wording::date_and_clock(screen.day(), now)),
    ]);
    let state = match screen.model_availability() {
        Some(bunshin_core::Availability::Unavailable(_)) => {
            Span::styled(wording::MODEL_UNAVAILABLE, ERROR)
        }
        Some(bunshin_core::Availability::Available) => match screen.chat_status(now.instant) {
            ChatStatus::Thinking | ChatStatus::LongWait => Span::styled(wording::THINKING, MODEL),
            ChatStatus::Waiting => Span::raw(wording::WAITING),
            ChatStatus::Idle => Span::raw("準備完了"),
        },
        None => Span::raw("準備中…"),
    };
    let [left_area, right_area] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(u16::try_from(state.width()).unwrap_or(u16::MAX)),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(left), left_area);
    frame.render_widget(
        Paragraph::new(Line::from(state)).alignment(Alignment::Right),
        right_area,
    );
}

fn draw_tasks(frame: &mut Frame, screen: &MainScreen, area: Rect) {
    let focused = screen.focus() == Focus::Tasks;
    let block = block(wording::TASKS_TITLE, focused).title_top(
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
            let selected = screen.selection() == Some(index);
            let marker = if selected { '>' } else { ' ' };
            let prefix = format!(
                "{marker}{} {} ",
                wording::status_mark(task.status),
                wording::task_time(task)
            );
            let line = format!("{prefix}{}", sanitize(&task.title));
            let style = if task.status == TaskStatus::Dropped {
                BASE.add_modifier(Modifier::CROSSED_OUT)
            } else {
                BASE
            };
            ListItem::new(truncate(&line, usize::from(inner.width))).style(style)
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(screen.selection());
    frame.render_stateful_widget(
        List::new(rows).highlight_style(BASE.add_modifier(Modifier::REVERSED)),
        inner,
        &mut state,
    );
}

fn draw_chat(
    frame: &mut Frame,
    screen: &MainScreen,
    now: Now,
    area: Rect,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
) -> (usize, usize) {
    let block = block(wording::CHAT_TITLE, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = screen
        .day()
        .messages()
        .iter()
        .map(|message| {
            let speaker = match message.author {
                Author::You => wording::OWNER,
                Author::Bunshin => wording::APP_NAME,
                Author::System => wording::SYSTEM,
            };
            let style = match message.kind {
                MessageKind::Error => ERROR,
                MessageKind::Change | MessageKind::Notice => SYSTEM,
                MessageKind::Reply | MessageKind::Unprompted => {
                    if message.author == Author::Bunshin {
                        MODEL
                    } else {
                        BASE
                    }
                }
            };
            let timestamp = wording::chat_timestamp(local_at(message.time));
            let content = message.change_set.as_ref().map_or_else(
                || message.text.clone(),
                |set| wording::chat_changes(set, local_at),
            );
            let content = if message.cancelled {
                format!("{content}{}", wording::CANCELLED_MARK)
            } else {
                content
            };
            Line::from(vec![
                Span::raw(format!("{timestamp} ")),
                Span::styled(format!("{speaker}  "), style.add_modifier(Modifier::BOLD)),
                Span::styled(sanitize(&content), style),
            ])
        })
        .collect::<Vec<_>>();
    if let Some(bunshin_core::screen::ScreenError::Day(error)) = screen.error() {
        lines.push(Line::styled(wording::day_error(error), ERROR));
    }
    match screen.chat_status(now.instant) {
        ChatStatus::Thinking => lines.push(Line::styled(
            format!("{}  {}", wording::APP_NAME, wording::THINKING_ROW),
            MODEL,
        )),
        ChatStatus::LongWait => lines.push(Line::styled(
            format!("{}  {}", wording::APP_NAME, wording::LONG_WAIT),
            MODEL,
        )),
        ChatStatus::Idle | ChatStatus::Waiting => {}
    }
    let count = lines.len();
    let scroll = u16::try_from(screen.chat_scroll_top()).unwrap_or(u16::MAX);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        inner,
    );
    (count, usize::from(inner.height))
}

fn draw_input(frame: &mut Frame, screen: &MainScreen, area: Rect) {
    let block = block(wording::INPUT_TITLE, screen.focus() == Focus::Input).title_bottom(
        Line::from(wording::input_count(
            screen.input().chars(),
            screen.input_limit(),
        ))
        .right_aligned(),
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(sanitize(screen.input().text())).wrap(Wrap { trim: false }),
        inner,
    );
    if screen.focus() == Focus::Input
        && inner.width > 0
        && inner.height > 0
        && !screen.is_confirming_quit()
    {
        let prefix = screen
            .input()
            .text()
            .chars()
            .take(screen.input().cursor())
            .collect::<String>();
        let columns = Line::from(sanitize(&prefix)).width();
        let width = usize::from(inner.width);
        let x = columns % width;
        let y = (columns / width).min(usize::from(inner.height.saturating_sub(1)));
        frame.set_cursor_position((
            inner.x + u16::try_from(x).unwrap_or_default(),
            inner.y + u16::try_from(y).unwrap_or_default(),
        ));
    }
}

fn draw_help(frame: &mut Frame) {
    let full = frame.area();
    let width = full.width.saturating_sub(2).min(90);
    let height = full.height.saturating_sub(2).min(12);
    let area = Rect::new(
        full.x + (full.width - width) / 2,
        full.y + (full.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, area);
    let block = block(wording::HELP_TITLE, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = vec![
        Line::from(wording::HELP_IME),
        Line::from(wording::HELP_MARKS),
    ];
    for binding in task_help() {
        lines.push(binding_line(binding));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
fn binding_line(binding: &KeyBinding) -> Line<'static> {
    let keys = binding
        .keys
        .iter()
        .map(|key| wording::key_label(*key))
        .collect::<Vec<_>>()
        .join("/");
    Line::from(format!("{keys}  {}", wording::action_label(binding.action)))
}
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
fn truncate(text: &str, width: usize) -> String {
    if Line::from(text).width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut used = 0_usize;
    let content_width = width.saturating_sub(1);
    for character in text.chars() {
        let character_width = Line::from(character.to_string()).width();
        if used.saturating_add(character_width) > content_width {
            break;
        }
        result.push(character);
        used += character_width;
    }
    result.push('…');
    result
}
