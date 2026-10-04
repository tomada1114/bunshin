//! Captured forms and help cover the main screen without owning domain state.
use super::{
    BASE_STYLE, ERROR_STYLE, FOCUSED_STYLE, KEY_STYLE, binding_line, compact_binding_line,
    region_block, truncate,
};
use crate::wording;
use bunshin_core::{
    day::TaskKind,
    screen::{
        Focus, MainScreen,
        help::help_rows,
        keys::KeyRegion,
        task_form::{FormField, TaskForm},
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};

pub(super) fn draw(frame: &mut Frame, screen: &MainScreen) {
    if screen.is_confirming_quit() {
        return;
    }
    match screen.focus() {
        Focus::Form => {
            if let Some(form) = screen.form() {
                draw_form(frame, form);
            }
        }
        Focus::Help => draw_help(frame),
        Focus::Instructions => draw_instructions(frame, screen),
        Focus::Input | Focus::Tasks => {}
    }
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
    let area = centered(frame.area(), 100, frame.area().height);
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

fn draw_instructions(frame: &mut Frame, screen: &MainScreen) {
    let area = centered(frame.area(), 100, frame.area().height);
    frame.render_widget(Clear, area);
    let Some(state) = screen.instructions() else {
        return;
    };
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
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((
            u16::try_from(screen.instructions_scroll()).unwrap_or(u16::MAX),
            0,
        )),
        body,
    );
    frame.render_widget(Paragraph::new(wording::INSTRUCTIONS_FOOTER), footer);
}
