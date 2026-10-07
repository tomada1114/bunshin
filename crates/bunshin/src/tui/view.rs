//! Draw the shared board without reading input or changing state.
#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;

use crate::wording;
use bunshin_core::{
    Now,
    board::Author,
    screen::{BoardFocus, BoardScreen, BoardStatus},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Wrap},
};

const BASE: Style = Style::new();
const FOCUSED: Style = BASE.fg(Color::Green).add_modifier(Modifier::BOLD);
const ERROR: Style = BASE.fg(Color::Red);
const CHARACTER: Style = BASE.fg(Color::Magenta);
const DIVIDER: Style = BASE.fg(Color::Magenta).add_modifier(Modifier::BOLD);

pub(super) fn draw_with_metrics(
    frame: &mut Frame,
    screen: &BoardScreen,
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

    let input_rows = input_lines(
        screen.input().text(),
        screen.input().cursor(),
        usize::from(area.width.saturating_sub(2)),
    )
    .0
    .len()
    .clamp(1, 3);
    let input_height = u16::try_from(input_rows).unwrap_or(3).saturating_add(2);
    let [header, board_area, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(input_height),
    ])
    .areas(area);

    draw_header(frame, screen, now, header);
    let metrics = draw_board(frame, screen, board_area, local_at);
    draw_input(frame, screen, input);
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

fn draw_header(frame: &mut Frame, screen: &BoardScreen, now: Now, area: Rect) {
    let left = Line::from(vec![
        Span::styled(
            format!(" {}  ", wording::APP_NAME),
            BASE.add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("{:02}:{:02}", now.local.hour(), now.local.minute())),
    ]);
    let status_value = screen.status(now.instant);
    let status = wording::board_status(status_value);
    let status_style = if matches!(status_value, BoardStatus::Failure(_)) {
        ERROR
    } else if status_value == BoardStatus::Writing {
        CHARACTER
    } else {
        BASE
    };
    let [left_area, right_area] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(u16::try_from(Line::from(status.as_str()).width()).unwrap_or(u16::MAX)),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(left), left_area);
    frame.render_widget(
        Paragraph::new(Line::styled(status, status_style)).alignment(Alignment::Right),
        right_area,
    );
}

fn draw_board(
    frame: &mut Frame,
    screen: &BoardScreen,
    area: Rect,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<Now>,
) -> (usize, usize) {
    let board_block = block(wording::BOARD_TITLE, screen.focus() == BoardFocus::Board);
    let inner = board_block.inner(area);
    frame.render_widget(board_block, area);
    if inner.width == 0 || inner.height == 0 {
        return (0, 0);
    }

    let posts = screen.board().posts();
    if posts.is_empty() {
        frame.render_widget(
            Paragraph::new(wording::EMPTY_BOARD).wrap(Wrap { trim: false }),
            inner,
        );
        return (1, usize::from(inner.height));
    }

    let first_unseen = screen.board_first_unseen_post();
    let new_posts = screen.board_new_posts();
    let mut rows = Vec::new();
    for (index, post) in posts.iter().enumerate() {
        if Some(index) == first_unseen {
            rows.push((
                Line::styled(wording::new_posts_divider(new_posts), DIVIDER),
                0,
            ));
        }
        let timestamp = wording::post_timestamp(local_at(post.at));
        let name = post.author.name();
        let style = match post.author {
            Author::Owner => BASE,
            Author::Character(_) => CHARACTER,
        };
        let prefix = format!("{timestamp} {name}  ");
        let indent = Line::from(prefix.as_str()).width();
        rows.push((
            Line::from(vec![
                Span::raw(format!("{timestamp} ")),
                Span::styled(format!("{name}  "), style.add_modifier(Modifier::BOLD)),
                Span::styled(sanitize(&post.body), style),
            ]),
            indent,
        ));
    }
    let rows = wrap_chat_rows(rows, usize::from(inner.width));
    let count = rows.len();
    let height = usize::from(inner.height);
    let end = count.saturating_sub(height);
    let start = if screen.board_follows_latest() {
        end
    } else {
        screen.board_scroll_top().min(end)
    };
    frame.render_widget(
        Paragraph::new(rows).scroll((u16::try_from(start).unwrap_or(u16::MAX), 0)),
        inner,
    );
    (count, height)
}

fn draw_input(frame: &mut Frame, screen: &BoardScreen, area: Rect) {
    let bottom = Line::from(vec![
        Span::raw(wording::input_hint(screen.focus())),
        Span::raw(wording::input_count(
            screen.input().chars(),
            screen.input_limit(),
        )),
    ])
    .right_aligned();
    let input_block =
        block(wording::INPUT_TITLE, screen.focus() == BoardFocus::Input).title_bottom(bottom);
    let inner = input_block.inner(area);
    frame.render_widget(input_block, area);
    let (rows, column, row) = input_lines(
        screen.input().text(),
        screen.input().cursor(),
        usize::from(inner.width),
    );
    if screen.focus() == BoardFocus::Input && inner.width > 0 && inner.height > 0 {
        let start = row
            .saturating_add(1)
            .saturating_sub(usize::from(inner.height));
        let visible = rows.into_iter().skip(start).collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(visible), inner);
        frame.set_cursor_position((
            inner.x + u16::try_from(column).unwrap_or_default(),
            inner.y + u16::try_from(row - start).unwrap_or_default(),
        ));
    } else {
        frame.render_widget(Paragraph::new(rows), inner);
    }
}

fn input_lines(text: &str, cursor: usize, width: usize) -> (Vec<Line<'static>>, usize, usize) {
    if width == 0 {
        return (vec![Line::default()], 0, 0);
    }
    let text = Span::raw(sanitize(text));
    let mut rows = vec![String::new()];
    let mut column = 0;
    let mut scalars = 0;
    let mut cursor_position = None;
    for grapheme in text.styled_graphemes(BASE) {
        let columns = Span::raw(grapheme.symbol).width();
        if column + columns > width {
            rows.push(String::new());
            column = 0;
        }
        let end = scalars + grapheme.symbol.chars().count();
        if cursor_position.is_none() && cursor < end {
            cursor_position = Some((column, rows.len() - 1));
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
    let (column, row) = cursor_position.unwrap_or((column, rows.len() - 1));
    (rows.into_iter().map(Line::from).collect(), column, row)
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
        let base = BASE.patch(line.style);
        for span in &line.spans {
            for grapheme in span.styled_graphemes(base) {
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

fn sanitize(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}
