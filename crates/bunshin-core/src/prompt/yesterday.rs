//! Yesterday's record as model input: written by core, never by the model.
use super::budget::estimate;
use crate::{
    Tuning,
    rhythm::record::{DayRecord, Tally},
};

// Titles are shortened to this many characters before any is left out.
const SHORTEST_TITLE: usize = 8;

/// The record's text within `Tuning.prompt.yesterday_tokens`: counts first, then the
/// titles, shortened and then left out (done first, open last) until it fits.
#[must_use]
pub fn yesterday_text(record: &DayRecord, tuning: Tuning) -> String {
    let bound = tuning.prompt.yesterday_tokens;
    let ratio = tuning.prompt.ascii_chars_per_token;
    let mut tallies = [
        record.done.clone(),
        record.carried_over.clone(),
        record.dropped.clone(),
        record.open.clone(),
    ];
    let longest = tallies
        .iter()
        .flat_map(|tally| &tally.titles)
        .map(|title| title.chars().count())
        .max()
        .unwrap_or(0);
    let mut limit = longest;
    let mut text = render(record, &tallies, limit);
    while estimate(&text, ratio) > bound && limit > SHORTEST_TITLE {
        limit -= 1;
        text = render(record, &tallies, limit);
    }
    for index in 0..tallies.len() {
        while estimate(&text, ratio) > bound && tallies[index].titles.pop().is_some() {
            text = render(record, &tallies, limit);
        }
    }
    let mut fitted = String::new();
    for character in text.chars() {
        fitted.push(character);
        if estimate(&fitted, ratio) > bound {
            fitted.pop();
            break;
        }
    }
    fitted
}

fn render(record: &DayRecord, tallies: &[Tally; 4], limit: usize) -> String {
    let parts = ["完了", "持ち越し", "やめた", "未完了"]
        .iter()
        .zip(tallies)
        .map(|(label, tally)| {
            let titles = tally
                .titles
                .iter()
                .map(|title| shorten(title, limit))
                .collect::<Vec<_>>();
            if titles.is_empty() {
                format!("{label}{}件", tally.count)
            } else {
                format!("{label}{}件（{}）", tally.count, titles.join("、"))
            }
        })
        .collect::<Vec<_>>();
    format!("{}の記録: {}", record.date, parts.join("、"))
}

fn shorten(title: &str, limit: usize) -> String {
    if title.chars().count() <= limit {
        title.to_owned()
    } else {
        title
            .chars()
            .take(limit.saturating_sub(1))
            .chain(['…'])
            .collect()
    }
}
