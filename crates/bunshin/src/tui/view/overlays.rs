//! Captured forms and help cover the main screen without owning domain state.
use super::{
    BASE_STYLE, ERROR_STYLE, FOCUSED_STYLE, KEY_STYLE, binding_line, region_block, truncate,
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
    );
    if form.kind() != TaskKind::Untimed {
        draw_field(
            frame,
            time_row,
            wording::FORM_TIME,
            form.time_text(),
            form.field() == FormField::Time,
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
    let (row, text) = match form.field() {
        FormField::Title => (title_row, form.title()),
        FormField::Time => (time_row, form.time_text()),
        FormField::Kind => return,
    };
    let prefix = text.chars().take(form.cursor()).collect::<String>();
    let offset = u16::try_from(Span::raw(prefix).width()).unwrap_or(u16::MAX);
    let x = row
        .x
        .saturating_add(10)
        .saturating_add(offset)
        .min(row.right().saturating_sub(1));
    frame.set_cursor_position((x, row.y));
}
fn draw_field(frame: &mut Frame, area: Rect, label: &str, text: &str, selected: bool) {
    let prefix = format!(
        "{label}{}",
        " ".repeat(10_usize.saturating_sub(Span::raw(label).width()))
    );
    let text = truncate(text, usize::from(area.width).saturating_sub(10));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(prefix, if selected { KEY_STYLE } else { BASE_STYLE }),
            Span::raw(text),
        ])),
        area,
    );
}
fn draw_help(frame: &mut Frame) {
    let area = centered(frame.area(), 100, frame.area().height);
    frame.render_widget(Clear, area);
    let mut lines = Vec::new();
    for region in [
        KeyRegion::Anywhere,
        KeyRegion::Main,
        KeyRegion::Tasks,
        KeyRegion::Form,
        KeyRegion::Help,
    ] {
        let bindings = help_rows()
            .iter()
            .filter(|binding| binding.region == region)
            .collect::<Vec<_>>();
        let mut spans = vec![Span::styled(
            format!("{}  ", wording::region_label(region)),
            KEY_STYLE,
        )];
        spans.extend(binding_line(&bindings).spans);
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
