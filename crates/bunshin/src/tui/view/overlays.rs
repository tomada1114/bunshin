//! Captured forms and help cover the main screen without owning domain state.
use super::{
    BASE_STYLE, ERROR_STYLE, FOCUSED_STYLE, KEY_STYLE, binding_line, compact_binding_line,
    region_block, sanitize, tag_label, tag_style, truncate, wrap_chat_rows,
};
use crate::wording;
use bunshin_core::{
    day::TaskKind,
    inbox::InboxView,
    screen::{
        Focus, MainScreen,
        help::{help_rows, inbox_help},
        keys::KeyRegion,
        task_form::{FormField, TaskForm},
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};

pub(super) fn draw(
    frame: &mut Frame,
    screen: &MainScreen,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<bunshin_core::Now>,
) -> Option<(usize, usize)> {
    if screen.is_confirming_quit() {
        return None;
    }
    if let Some(inbox) = screen.inbox() {
        draw_inbox(frame, screen, &inbox, local_at);
        return None;
    }
    match screen.focus() {
        Focus::Form => {
            if let Some(form) = screen.form() {
                draw_form(frame, form);
            }
        }
        Focus::Help => draw_help(frame),
        Focus::Instructions => return draw_instructions(frame, screen),
        Focus::Input | Focus::Tasks => {}
    }
    None
}
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
fn draw_form(frame: &mut Frame, form: &TaskForm) {
    let area = centered(frame.area(), 64, 11);
    frame.render_widget(Clear, area);
    let title = if form.number().is_some() {
        wording::EDIT_TASK
    } else {
        wording::ADD_TASK
    };
    let block = region_block(title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [_, title_row, kind_row, time_row, _, error, _, help] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(inner);
    draw_field(
        frame,
        title_row,
        wording::FORM_TITLE,
        form.title(),
        form.field() == FormField::Title,
        (form.field() == FormField::Title).then(|| form.cursor()),
    );
    let kinds = [TaskKind::Untimed, TaskKind::Deadline, TaskKind::Appointment]
        .into_iter()
        .map(|kind| {
            format!(
                "({}) {}",
                if form.kind() == kind { '*' } else { ' ' },
                wording::kind_label(kind)
            )
        })
        .collect::<Vec<_>>()
        .join("  ");
    draw_field(
        frame,
        kind_row,
        wording::FORM_KIND,
        &kinds,
        form.field() == FormField::Kind,
        None,
    );
    if form.kind() != TaskKind::Untimed {
        draw_field(
            frame,
            time_row,
            wording::FORM_TIME,
            form.time_text(),
            form.field() == FormField::Time,
            (form.field() == FormField::Time).then(|| form.cursor()),
        );
    }
    if let Some(reason) = form.error() {
        frame.render_widget(
            Paragraph::new(wording::form_error(reason)).style(ERROR_STYLE),
            error,
        );
    }
    let keys = help_rows()
        .iter()
        .filter(|binding| {
            matches!(binding.region, KeyRegion::Form)
                && matches!(
                    binding.action,
                    bunshin_core::screen::keys::ScreenAction::SaveForm
                        | bunshin_core::screen::keys::ScreenAction::NextField
                        | bunshin_core::screen::keys::ScreenAction::CancelForm
                )
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(binding_line(&keys)), help);
}

fn draw_field(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    text: &str,
    selected: bool,
    cursor: Option<usize>,
) {
    let prefix = format!(
        "{label}{}",
        " ".repeat(10_usize.saturating_sub(Span::raw(label).width()))
    );
    let width = usize::from(area.width).saturating_sub(10);
    let (text, offset) = cursor.map_or_else(
        || (truncate(text, width), None),
        |cursor| {
            let (text, offset) = field_window(text, cursor, width);
            (text, Some(offset))
        },
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(prefix, if selected { KEY_STYLE } else { BASE_STYLE }),
            Span::raw(text),
        ])),
        area,
    );
    if let Some(offset) = offset
        && width > 0
    {
        let offset = u16::try_from(offset).unwrap_or_default();
        frame.set_cursor_position((area.x.saturating_add(10).saturating_add(offset), area.y));
    }
}
pub(super) fn field_window(text: &str, cursor: usize, width: usize) -> (String, usize) {
    if width == 0 {
        return (String::new(), 0);
    }
    let text = text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let span = Span::raw(&text);
    let graphemes = span
        .styled_graphemes(BASE_STYLE)
        .map(|g| g.symbol.to_owned())
        .collect::<Vec<_>>();
    let mut chars = 0;
    let mut cursor_index = graphemes.len();
    for (index, grapheme) in graphemes.iter().enumerate() {
        let end = chars + grapheme.chars().count();
        if cursor < end {
            cursor_index = index;
            break;
        }
        chars = end;
    }
    let cursor_column = graphemes
        .iter()
        .take(cursor_index)
        .map(|g| Span::raw(g).width())
        .sum::<usize>();
    let desired = if Span::raw(&text).width() < width {
        0
    } else {
        cursor_column.saturating_sub(width / 2)
    };
    let mut start = 0;
    let mut skipped = 0;
    while start < cursor_index && skipped < desired {
        skipped += Span::raw(&graphemes[start]).width();
        start += 1;
    }
    let marker = usize::from(start > 0 && width > 1);
    let mut visible = if marker > 0 {
        "…".to_string()
    } else {
        String::new()
    };
    // Reserve the final cell for an insertion point or a right overflow marker.
    let mut columns = marker;
    let mut end = start;
    while let Some(grapheme) = graphemes.get(end) {
        let size = Span::raw(grapheme).width();
        if columns + size > width.saturating_sub(1) {
            break;
        }
        visible.push_str(grapheme);
        columns += size;
        end += 1;
    }
    let offset = marker + cursor_column.saturating_sub(skipped);
    if end < graphemes.len() {
        visible.push('…');
    }
    (visible, offset.min(width - 1))
}
fn draw_help(frame: &mut Frame) {
    // Every group must fit 60×18, so the overlay takes the full height.
    let full = frame.area();
    let width = full.width.saturating_sub(2).min(100);
    let area = Rect::new(
        full.x + (full.width - width) / 2,
        full.y,
        width,
        full.height,
    );
    frame.render_widget(Clear, area);
    let mut lines = Vec::new();
    for region in [
        KeyRegion::Anywhere,
        KeyRegion::Main,
        KeyRegion::Input,
        KeyRegion::Tasks,
        KeyRegion::Form,
        KeyRegion::Help,
        KeyRegion::Instructions,
    ] {
        let bindings = help_rows()
            .iter()
            .filter(|binding| binding.region == region)
            .collect::<Vec<_>>();
        let mut spans = vec![Span::styled(
            format!("{}  ", wording::region_label(region)),
            KEY_STYLE,
        )];
        spans.extend(compact_binding_line(&bindings).spans);
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(wording::HELP_IME));
    lines.push(Line::from(wording::HELP_MARKS));
    lines.push(Line::from(wording::HELP_TIMES));
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(region_block(wording::HELP_TITLE, true).title_style(FOCUSED_STYLE)),
        area,
    );
}

fn draw_instructions(frame: &mut Frame, screen: &MainScreen) -> Option<(usize, usize)> {
    let area = centered(frame.area(), 100, frame.area().height);
    frame.render_widget(Clear, area);
    let state = screen.instructions()?;
    let counter = if state.origin == bunshin_core::instructions::InstructionsOrigin::Owner {
        format!(
            " {}/{}字 ",
            state.file_chars.unwrap_or_default(),
            state.limit
        )
    } else {
        wording::INSTRUCTIONS_DEFAULT.to_owned()
    };
    let block = region_block(wording::INSTRUCTIONS_TITLE, true)
        .title_top(Line::from(counter).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [body, footer] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    let mut lines = Vec::new();
    if state.origin != bunshin_core::instructions::InstructionsOrigin::Owner {
        lines.push(Line::from(state.failure.map_or_else(
            || wording::instructions_source(state),
            wording::instructions_error,
        )));
        lines.push(Line::default());
    }
    lines.extend(state.text.lines().map(|line| Line::from(line.to_owned())));
    lines.push(Line::default());
    lines.push(Line::from(wording::instructions_path(&state.path)));
    lines.push(Line::from(wording::INSTRUCTIONS_EDIT));
    let lines = super::wrap_chat_rows(
        lines.into_iter().map(|line| (line, 0)).collect(),
        usize::from(body.width),
    );
    let metrics = (lines.len(), usize::from(body.height));
    let scroll = screen
        .instructions_scroll()
        .min(metrics.0.saturating_sub(metrics.1));
    frame.render_widget(
        Paragraph::new(lines).scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
        body,
    );
    frame.render_widget(Paragraph::new(wording::INSTRUCTIONS_FOOTER), footer);
    Some(metrics)
}

/// T4: today's open notes and questions, newest first, with its own key line.
fn draw_inbox(
    frame: &mut Frame,
    screen: &MainScreen,
    inbox: &InboxView,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<bunshin_core::Now>,
) {
    let width = 80.min(frame.area().width.saturating_sub(2));
    let text_width = usize::from(width.saturating_sub(2));
    let mut rows = vec![(Line::default(), 0)];
    let mut selected_row = 0;
    if inbox.items.is_empty() {
        rows.push((Line::from(format!(" {}", wording::INBOX_EMPTY)), 1));
    }
    for (index, item) in inbox.items.iter().enumerate() {
        let chosen = inbox.selected == Some(index);
        let marker = if chosen { '>' } else { ' ' };
        let clock = wording::chat_timestamp(local_at(item.time));
        let mut first = Line::from(vec![
            Span::raw(format!(" {marker} {clock} ")),
            Span::styled(
                tag_label(screen, item.kind, &item.trigger, item.task),
                tag_style(item.kind),
            ),
        ]);
        if chosen {
            selected_row = rows.len();
            first = first.patch_style(BASE_STYLE.add_modifier(Modifier::REVERSED));
        }
        rows.push((first, 9));
        for part in item.text.lines() {
            rows.push((
                Line::from(format!("{}{}", " ".repeat(9), sanitize(part))),
                9,
            ));
        }
    }
    rows.push((Line::default(), 0));
    if !inbox.items.is_empty() {
        rows.push((
            Line::from(format!(
                "   {}",
                wording::inbox_note(screen.tuning().inbox_reaction_minutes)
            )),
            3,
        ));
        rows.push((Line::default(), 0));
    }
    let lines = wrap_chat_rows(rows, text_width);
    let wanted = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(3);
    let area = centered(frame.area(), width, wanted);
    frame.render_widget(Clear, area);
    let block = region_block(wording::INBOX_TITLE, true)
        .title_top(Line::from(wording::inbox_count(inbox.items.len())).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [body, footer] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    let scroll = (selected_row + 2).saturating_sub(usize::from(body.height));
    frame.render_widget(
        Paragraph::new(lines).scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
        body,
    );
    let keys = inbox_help();
    let keys = if inbox.items.is_empty() {
        keys.into_iter()
            .filter(|binding| {
                binding.action == bunshin_core::screen::keys::ScreenAction::CloseInbox
            })
            .collect()
    } else {
        keys
    };
    frame.render_widget(Paragraph::new(first_key_line(&keys)), footer);
}
/// A key line naming only each binding's first key, as T4's own line does.
fn first_key_line(bindings: &[&bunshin_core::screen::keys::KeyBinding]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for binding in bindings {
        if let Some(key) = binding.keys.first() {
            spans.push(Span::styled(wording::key_label(*key), KEY_STYLE));
            spans.push(Span::raw(format!(
                " {}  ",
                wording::action_label(binding.action)
            )));
        }
    }
    Line::from(spans)
}
