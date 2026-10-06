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
    day::{Author, MessageKind, TaskStatus},
    screen::{
        ChatStatus, Focus, MainScreen, SaveState, ScreenError,
        help::{help_rows, leftovers_help, task_help},
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
const BUNSHIN_STYLE: Style = BASE_STYLE.fg(Color::Magenta);
const SYSTEM_STYLE: Style = BASE_STYLE.fg(Color::Blue);
const INBOX_STYLE: Style = BUNSHIN_STYLE.add_modifier(Modifier::BOLD);
const NOTE_TAG_STYLE: Style = BUNSHIN_STYLE;
const QUESTION_TAG_STYLE: Style = BUNSHIN_STYLE.add_modifier(Modifier::BOLD);
const DIVIDER_STYLE: Style = BUNSHIN_STYLE;
const LEFTOVERS_HEADING_STYLE: Style = BASE_STYLE.add_modifier(Modifier::BOLD);

#[cfg(test)]
pub fn draw(frame: &mut Frame, screen: &MainScreen, now: Now) {
    draw_with_metrics(frame, screen, now, &|at| now.at_fixed_offset(at));
}
pub(super) fn draw_with_metrics(
    frame: &mut Frame,
    screen: &MainScreen,
    now: Now,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
) -> (usize, usize, Option<(usize, usize)>) {
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
        return (0, 0, None);
    }
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(frame, screen, now, local_at, header);
    let (tasks, right) = if area.width >= 100 {
        let [tasks, right] =
            Layout::horizontal([Constraint::Length(36), Constraint::Min(0)]).areas(body);
        (tasks, right)
    } else {
        let rows = u16::try_from(task_pane_rows(screen))
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .clamp(3, 12)
            .min(body.height.saturating_sub(6));
        let [tasks, right] =
            Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).areas(body);
        (tasks, right)
    };
    draw_tasks(frame, screen, tasks);
    let rows = input_lines(screen, usize::from(right.width.saturating_sub(2)))
        .0
        .len()
        .clamp(1, 3);
    let input_height = u16::try_from(rows).unwrap_or(3).saturating_add(2);
    let [chat, input] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(input_height)]).areas(right);
    let metrics = draw_chat(frame, screen, now, chat, local_at);
    draw_input(frame, screen, now, local_at, input);
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
            Paragraph::new(binding_line(&footer_bindings(screen))),
            footer,
        );
    }
    let instructions = overlays::draw(frame, screen, local_at);
    (metrics.0, metrics.1, instructions)
}
fn draw_header(
    frame: &mut Frame,
    screen: &MainScreen,
    now: Now,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
    area: Rect,
) {
    let left = Line::from(vec![
        Span::styled(format!(" {}  ", wording::APP_NAME), KEY_STYLE),
        Span::raw(wording::date_and_clock(screen.day(), now)),
    ]);
    let room = usize::from(area.width).saturating_sub(left.width() + 1);
    let right = header_items(screen, now, local_at, room);
    let width = u16::try_from(right.width()).unwrap_or(u16::MAX);
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(width)]).areas(area);
    frame.render_widget(Paragraph::new(left), left_area);
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        right_area,
    );
}
/// The header's status items in the product order, each only when it applies.
fn header_items(
    screen: &MainScreen,
    now: Now,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
    room: usize,
) -> Line<'static> {
    let checkins = screen.checkin_header(now);
    let mut items = Vec::new();
    if let Some(count) = checkins.inbox {
        items.push((Span::styled(wording::header_inbox(count), INBOX_STYLE), 4));
    }
    if let Some(count) = checkins.held {
        items.push((Span::styled(wording::header_held(count), KEY_STYLE), 3));
    }
    match screen.save_state() {
        SaveState::Saved => {}
        SaveState::NotSaved(_) => {
            items.push((Span::styled(wording::NOT_SAVED, ERROR_LABEL_STYLE), 5));
        }
        SaveState::DurabilityUnconfirmed => {
            items.push((Span::styled(wording::NOT_DURABLE, ERROR_LABEL_STYLE), 5));
        }
    }
    if let Some(until) = checkins.muted_until {
        let until = wording::chat_timestamp(local_at(until));
        items.push((Span::styled(wording::header_muted(&until), KEY_STYLE), 2));
    }
    if let Some(start) = checkins.outside_hours_until {
        items.push((
            Span::styled(
                wording::header_outside_hours(start.hour(), start.minute()),
                KEY_STYLE,
            ),
            1,
        ));
    }
    items.push((
        Span::raw(wording::header_next_look(
            checkins.next_look.map(|at| (at.hour(), at.minute())),
        )),
        0,
    ));
    let unavailable = matches!(
        screen.model_availability(),
        Some(bunshin_core::Availability::Unavailable(_))
    );
    let model = if unavailable {
        Span::styled(wording::MODEL_UNAVAILABLE, ERROR_LABEL_STYLE)
    } else {
        match screen.chat_status(now.instant) {
            ChatStatus::Thinking | ChatStatus::LongWait => {
                Span::styled(wording::THINKING, BUNSHIN_STYLE)
            }
            ChatStatus::Waiting | ChatStatus::Idle if checkins.checking_in => {
                Span::styled(wording::CHECKING_IN, BUNSHIN_STYLE)
            }
            ChatStatus::Waiting | ChatStatus::Idle => Span::raw(wording::WAITING),
        }
    };
    items.push((model, 6));
    // A narrow header keeps the date and clock: the least urgent items go first.
    let separator = Span::raw(wording::HEADER_SEPARATOR).width();
    while items.len() > 1
        && items.iter().map(|(span, _)| span.width()).sum::<usize>() + separator * (items.len() - 1)
            > room
    {
        if let Some(lowest) = items
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, priority))| *priority)
            .map(|(index, _)| index)
        {
            items.remove(lowest);
        }
    }
    let mut spans = Vec::new();
    for (index, (item, _)) in items.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(wording::HEADER_SEPARATOR));
        }
        spans.push(item);
    }
    Line::from(spans)
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
    let width = usize::from(inner.width);
    let tasks = screen.day().task_view();
    let leftovers = screen.leftovers();
    let mut rows = Vec::new();
    let mut selected = None;
    if leftovers.is_empty() {
        if tasks.is_empty() {
            frame.render_widget(
                Paragraph::new(wording::EMPTY_TASKS).wrap(Wrap { trim: false }),
                inner,
            );
            return;
        }
    } else {
        if let Some(previous) = screen.leftovers_day() {
            let heading = format!("  {}", wording::leftovers_heading(previous));
            rows.push(ListItem::new(truncate(&heading, width)).style(LEFTOVERS_HEADING_STYLE));
        }
        for (index, task) in leftovers.iter().enumerate() {
            let chosen = screen.leftover_selection() == Some(index);
            if chosen {
                selected = Some(rows.len());
            }
            rows.push(task_row(task, chosen, None, width));
        }
        rows.push(ListItem::new(format!(
            "  {}",
            "─".repeat(width.saturating_sub(4))
        )));
        if tasks.is_empty() {
            rows.push(ListItem::new(format!("  {}", wording::LEFTOVERS_NO_TASKS)));
        }
    }
    let offset = rows.len();
    for (index, task) in tasks.iter().enumerate() {
        let chosen = screen.selection() == Some(index);
        if chosen {
            selected = Some(offset + index);
        }
        rows.push(task_row(task, chosen, Some(task.number), width));
    }
    let mut selection = ListState::default().with_selected(selected);
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
/// The rows the task pane needs: the leftovers block, when shown, sits above the tasks.
fn task_pane_rows(screen: &MainScreen) -> usize {
    let tasks = screen.day().tasks().len();
    let leftovers = screen.leftovers().len();
    if leftovers == 0 {
        tasks
    } else {
        leftovers + 2 + tasks.max(1)
    }
}
/// One task row; a leftover is listed without today's number, since it has none yet.
fn task_row(
    task: &bunshin_core::day::TaskView,
    chosen: bool,
    number: Option<u64>,
    width: usize,
) -> ListItem<'static> {
    let marker = if chosen { '>' } else { ' ' };
    let time = wording::task_time(task);
    let time_width = Span::raw(&time).width();
    let number = number.map_or_else(String::new, |number| number.to_string());
    let prefix = format!(
        "{marker}{number} {} {time}{} ",
        wording::status_mark(task.status),
        " ".repeat(7_usize.saturating_sub(time_width))
    );
    let prefix_width = Span::raw(&prefix).width();
    let title = truncate(&task.title, width.saturating_sub(prefix_width));
    let style = if task.status == TaskStatus::Dropped {
        DROPPED_STYLE
    } else {
        BASE_STYLE
    };
    ListItem::new(format!("{prefix}{title}")).style(style)
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
fn draw_input(
    frame: &mut Frame,
    screen: &MainScreen,
    now: Now,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
    area: Rect,
) {
    let target = screen
        .reply_target()
        .or_else(|| screen.day().implicit_reply_target(now.instant));
    let time = target.map(|target| {
        wording::chat_timestamp(usize::try_from(target).ok().and_then(|index| {
            screen
                .day()
                .messages()
                .get(index)
                .and_then(|message| local_at(message.time))
        }))
    });
    let title = wording::input_title(time.as_deref());
    let counter = wording::input_count(screen.input().chars(), screen.input_limit());
    let counter_style = if screen.input().at_limit(screen.input_limit()) {
        ERROR_LABEL_STYLE
    } else {
        BASE_STYLE
    };
    let block = region_block(&title, screen.focus() == Focus::Input)
        .title_bottom(Line::from(Span::styled(counter, counter_style)).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let (rows, column, row) = input_lines(screen, usize::from(inner.width));
    let start = row
        .saturating_add(1)
        .saturating_sub(usize::from(inner.height));
    frame.render_widget(
        Paragraph::new(rows.into_iter().skip(start).collect::<Vec<_>>()),
        inner,
    );
    if screen.focus() == Focus::Input && !screen.is_confirming_quit() {
        frame.set_cursor_position((
            inner.x + u16::try_from(column).unwrap_or_default(),
            inner.y + u16::try_from(row - start).unwrap_or_default(),
        ));
    }
}
fn input_lines(screen: &MainScreen, width: usize) -> (Vec<Line<'static>>, usize, usize) {
    if width == 0 {
        return (vec![Line::default()], 0, 0);
    }
    let text = Span::raw(screen.input().text());
    let mut rows = vec![String::new()];
    let mut column = 0;
    let mut scalars = 0;
    let mut cursor = None;
    for grapheme in text.styled_graphemes(BASE_STYLE) {
        let columns = Span::raw(grapheme.symbol).width();
        if column + columns > width {
            rows.push(String::new());
            column = 0;
        }
        let end = scalars + grapheme.symbol.chars().count();
        if cursor.is_none() && screen.input().cursor() < end {
            cursor = Some((column, rows.len() - 1));
        }
        if let Some(row) = rows.last_mut() {
            row.push_str(grapheme.symbol);
        }
        column += columns;
        scalars = end;
    }
    if column == width {
        rows.push(String::new());
        column = 0;
    }
    let (column, row) = cursor.unwrap_or((column, rows.len() - 1));
    (rows.into_iter().map(Line::from).collect(), column, row)
}
fn draw_chat(
    frame: &mut Frame,
    screen: &MainScreen,
    now: Now,
    area: Rect,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
) -> (usize, usize) {
    let mut block = region_block(wording::CHAT_TITLE, false);
    if screen.chat_new_messages() > 0 {
        block = block.title_bottom(
            Line::from(wording::chat_new_rows(screen.chat_new_messages())).right_aligned(),
        );
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut rows = persisted_chat_rows(screen, local_at);
    if let Some(ScreenError::Day(error)) = screen.error() {
        rows.push((
            Line::from(vec![
                Span::styled(format!("{}  ", wording::ERROR_SPEAKER), ERROR_LABEL_STYLE),
                Span::styled(wording::day_error(error), ERROR_STYLE),
            ]),
            8,
        ));
    }
    match screen.chat_status(now.instant) {
        ChatStatus::Thinking => rows.push((
            Line::styled(
                format!("{}  {}", wording::APP_NAME, wording::THINKING_ROW),
                BUNSHIN_STYLE,
            ),
            0,
        )),
        ChatStatus::LongWait => rows.push((
            Line::styled(
                format!("{}  {}", wording::APP_NAME, wording::LONG_WAIT),
                BUNSHIN_STYLE,
            ),
            0,
        )),
        ChatStatus::Idle | ChatStatus::Waiting => {}
    }
    // Wrap by measured grapheme widths so the complete history has a stable row count
    // and long responses can later be scrolled without truncating stored text.
    let rows = wrap_chat_rows(rows, usize::from(inner.width));
    let metrics = (rows.len(), usize::from(inner.height));
    let last = rows.len().saturating_sub(usize::from(inner.height));
    let start = if screen.chat_follows_latest() {
        last
    } else {
        screen.chat_scroll_top().min(last)
    };
    frame.render_widget(
        Paragraph::new(rows.into_iter().skip(start).collect::<Vec<_>>()),
        inner,
    );
    metrics
}
fn persisted_chat_rows(
    screen: &MainScreen,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
) -> Vec<(Line<'static>, usize)> {
    let mut rows = Vec::new();
    let unseen = screen.chat_first_unseen();
    for (index, message) in screen.day().messages().iter().enumerate() {
        if message
            .unprompted
            .as_ref()
            .is_some_and(|extra| extra.suppressed.is_some())
        {
            continue;
        }
        if unseen == Some(index) {
            rows.push((Line::styled(wording::NEW_MESSAGE_DIVIDER, DIVIDER_STYLE), 0));
        }
        let speaker = if message.kind == MessageKind::Change {
            wording::CHANGE
        } else if message.kind == MessageKind::Error {
            wording::ERROR_SPEAKER
        } else {
            match message.author {
                Author::You => wording::OWNER,
                Author::Bunshin => wording::APP_NAME,
                Author::System => wording::SYSTEM,
            }
        };
        let style = if message.kind == MessageKind::Error {
            ERROR_STYLE
        } else if message.kind == MessageKind::Change || message.author == Author::System {
            SYSTEM_STYLE
        } else if message.author == Author::Bunshin {
            BUNSHIN_STYLE
        } else {
            BASE_STYLE
        };
        let text = message.change_set.as_ref().map_or_else(
            || message.text.clone(),
            |set| wording::chat_changes(set, local_at),
        );
        let text = if message.cancelled {
            format!("{text}{}", wording::CANCELLED_MARK)
        } else {
            text
        };
        let clock = wording::chat_timestamp(local_at(message.time));
        let speaker = format!(
            "{speaker}{}",
            " ".repeat(8_usize.saturating_sub(Span::raw(speaker).width()))
        );
        let prefix = format!("{clock} {speaker}");
        let indent = Span::raw(&prefix).width();
        let mut parts = text.lines();
        let first = unprompted_tag(screen, message)
            .unwrap_or_else(|| Span::styled(sanitize(parts.next().unwrap_or_default()), style));
        rows.push((
            Line::from(vec![
                Span::styled(format!("{clock} "), BASE_STYLE),
                Span::styled(speaker, style.add_modifier(Modifier::BOLD)),
                first,
            ]),
            indent,
        ));
        for part in parts {
            rows.push((
                Line::from(vec![
                    Span::raw(" ".repeat(indent)),
                    Span::styled(sanitize(part), style),
                ]),
                indent,
            ));
        }
    }
    rows
}
/// An unprompted message opens with its tag line, so its kind and reason read without
/// color; its text follows on the rows below.
fn unprompted_tag(
    screen: &MainScreen,
    message: &bunshin_core::day::Message,
) -> Option<Span<'static>> {
    let extra = message.unprompted.as_ref()?;
    Some(Span::styled(tag_text(screen, extra), tag_style(extra.kind)))
}
fn tag_text(screen: &MainScreen, extra: &bunshin_core::day::UnpromptedMessage) -> String {
    tag_label(screen, extra.kind, &extra.trigger, extra.task)
}
fn tag_label(
    screen: &MainScreen,
    kind: bunshin_core::day::UnpromptedKind,
    trigger: &bunshin_core::day::Trigger,
    task: Option<u64>,
) -> String {
    let title = task.or(trigger.task).and_then(|number| {
        screen
            .day()
            .tasks()
            .iter()
            .find(|task| task.number == number)
            .map(|task| task.title.as_str())
    });
    wording::unprompted_label(
        kind,
        trigger,
        title,
        screen.tuning().checkin.before_deadline_minutes,
    )
}
const fn tag_style(kind: bunshin_core::day::UnpromptedKind) -> Style {
    match kind {
        bunshin_core::day::UnpromptedKind::Note => NOTE_TAG_STYLE,
        bunshin_core::day::UnpromptedKind::Question => QUESTION_TAG_STYLE,
    }
}
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}
fn wrap_chat_rows(rows: Vec<(Line<'static>, usize)>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let mut wrapped = Vec::new();
    for (line, indent) in rows {
        let indent = indent.min(width.saturating_sub(1));
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut used = 0;
        for span in &line.spans {
            for grapheme in span.styled_graphemes(BASE_STYLE) {
                let size = Span::raw(grapheme.symbol).width();
                if used + size > width && !spans.is_empty() {
                    wrapped.push(Line::from(std::mem::take(&mut spans)));
                    spans.push(Span::raw(" ".repeat(indent)));
                    used = indent;
                }
                if let Some(last) = spans.last_mut()
                    && last.style == grapheme.style
                {
                    last.content.to_mut().push_str(grapheme.symbol);
                } else {
                    spans.push(Span::styled(grapheme.symbol.to_owned(), grapheme.style));
                }
                used += size;
            }
        }
        wrapped.push(Line::from(spans));
    }
    wrapped
}
fn footer_bindings(screen: &MainScreen) -> Vec<&'static KeyBinding> {
    let focus = screen.focus();
    if focus == Focus::Tasks {
        if screen.leftover_selection().is_some() {
            return leftovers_help();
        }
        return task_help();
    }
    let region = match focus {
        Focus::Input => KeyRegion::Input,
        Focus::Tasks => KeyRegion::Tasks,
        Focus::Form => KeyRegion::Form,
        Focus::Help => KeyRegion::Help,
        Focus::Instructions => KeyRegion::Instructions,
    };
    let mut bindings = help_rows()
        .iter()
        .filter(|binding| {
            binding.region == region
                || binding.region == KeyRegion::Anywhere
                || (focus == Focus::Input && binding.region == KeyRegion::Main)
        })
        .collect::<Vec<_>>();
    let first = [
        bunshin_core::screen::keys::ScreenAction::SendInput,
        bunshin_core::screen::keys::ScreenAction::CancelInput,
        bunshin_core::screen::keys::ScreenAction::Quit,
        bunshin_core::screen::keys::ScreenAction::Undo,
        bunshin_core::screen::keys::ScreenAction::MoveFocus,
    ];
    bindings.sort_by_key(|binding| {
        first
            .iter()
            .position(|action| *action == binding.action)
            .unwrap_or(first.len())
    });
    bindings
}
fn binding_line(bindings: &[&KeyBinding]) -> Line<'static> {
    binding_line_with_gap(bindings, 2)
}
fn compact_binding_line(bindings: &[&KeyBinding]) -> Line<'static> {
    binding_line_with_gap(bindings, 1)
}
fn binding_line_with_gap(bindings: &[&KeyBinding], gap: usize) -> Line<'static> {
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
            " {}{}",
            wording::action_label(binding.action),
            " ".repeat(gap)
        )));
    }
    Line::from(spans)
}
