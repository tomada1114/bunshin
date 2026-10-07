//! Board-screen wording, separate from terminal drawing and core transitions.
use bunshin_core::{
    ModelError, Now, UnavailableReason,
    board::FailureKind,
    screen::{
        BoardFailure, BoardFocus, BoardStatus, KEY_TABLE, KeyRegion, ScreenAction, ScreenKey,
    },
};

pub const APP_NAME: &str = "Bunshin";
pub const BOARD_TITLE: &str = " 掲示板 ";
pub const INPUT_TITLE: &str = " 入力 ";
pub const EMPTY_BOARD: &str = "まだ投稿はありません";
pub const TOO_SMALL: &str = "端末が小さすぎます";
pub const EXPAND_TERMINAL: &str = "60×18 以上に広げてください";
pub const IDLE: &str = "待機中";
pub const WRITING: &str = "書き込み中…";

pub fn input_hint(focus: BoardFocus) -> String {
    let region = match focus {
        BoardFocus::Input => KeyRegion::Input,
        BoardFocus::Board => KeyRegion::Board,
    };
    let mut grouped = Vec::<(ScreenAction, Vec<ScreenKey>)>::new();
    for binding in KEY_TABLE
        .iter()
        .filter(|binding| binding.region == region || binding.region == KeyRegion::Anywhere)
    {
        if let Some((_, keys)) = grouped
            .iter_mut()
            .find(|(action, _)| *action == binding.action)
        {
            keys.extend(binding.keys.iter().copied());
        } else {
            grouped.push((binding.action, binding.keys.to_vec()));
        }
    }
    grouped
        .into_iter()
        .map(|(action, keys)| {
            let keys = keys
                .iter()
                .map(|key| key_label(*key))
                .collect::<Vec<_>>()
                .join("/");
            format!("{keys} {}", action_label(action, focus))
        })
        .collect::<Vec<_>>()
        .join("  ")
}

fn key_label(key: ScreenKey) -> String {
    match key {
        ScreenKey::Char(character) => character.to_string(),
        ScreenKey::Up => "↑".to_owned(),
        ScreenKey::Down => "↓".to_owned(),
        ScreenKey::Left => "←".to_owned(),
        ScreenKey::Right => "→".to_owned(),
        ScreenKey::Enter => "Enter".to_owned(),
        ScreenKey::Tab => "Tab".to_owned(),
        ScreenKey::BackTab => "⇧Tab".to_owned(),
        ScreenKey::Esc => "Esc".to_owned(),
        ScreenKey::Interrupt => "Ctrl+C".to_owned(),
        ScreenKey::Backspace => "BS".to_owned(),
        ScreenKey::Delete => "Del".to_owned(),
        ScreenKey::Home => "Home".to_owned(),
        ScreenKey::End => "End".to_owned(),
        ScreenKey::PageUp => "PgUp".to_owned(),
        ScreenKey::PageDown => "PgDn".to_owned(),
    }
}

fn action_label(action: ScreenAction, focus: BoardFocus) -> &'static str {
    match action {
        ScreenAction::Quit => "終了",
        ScreenAction::ToggleFocus => match focus {
            BoardFocus::Input => "掲示板へ",
            BoardFocus::Board => "入力へ",
        },
        ScreenAction::ScrollOlder => "前へ",
        ScreenAction::ScrollNewer => "次へ",
        ScreenAction::Latest => "最新へ",
        ScreenAction::FocusInput => "入力へ",
        ScreenAction::SubmitInput => "投稿",
        ScreenAction::ClearInput => "消去",
    }
}

pub fn input_count(chars: usize, limit: usize) -> String {
    format!(" {chars}/{limit} ")
}

pub fn new_posts_divider(count: usize) -> String {
    format!("── 新着 {count}件 ──")
}

pub fn post_timestamp(now: Option<Now>) -> String {
    now.map_or_else(
        || "--:--".to_owned(),
        |now| format!("{:02}:{:02}", now.local.hour(), now.local.minute()),
    )
}

pub fn board_status(status: BoardStatus) -> String {
    match status {
        BoardStatus::Idle => IDLE.to_owned(),
        BoardStatus::Writing => WRITING.to_owned(),
        BoardStatus::Failure(failure) => board_failure(failure),
    }
}

pub fn board_failure(failure: BoardFailure) -> String {
    match failure {
        BoardFailure::Model(error) => match error {
            ModelError::Unavailable(reason) => unavailable(reason).to_owned(),
            ModelError::TimedOut => "応答がタイムアウトしました".to_owned(),
            ModelError::Cancelled => "応答を中止しました".to_owned(),
            ModelError::Refused => "応答できませんでした".to_owned(),
            ModelError::Malformed => "応答を読み取れませんでした".to_owned(),
            ModelError::Failed => "モデル応答に失敗しました".to_owned(),
        },
        BoardFailure::Kind(kind) => failure_kind(kind).to_owned(),
    }
}

fn unavailable(reason: UnavailableReason) -> &'static str {
    match reason {
        UnavailableReason::UnsupportedOs => "この環境ではモデルを利用できません",
        UnavailableReason::NotInstalled => "fm が見つかりません",
        UnavailableReason::TermsNotAccepted => "モデルの利用条件への同意が必要です",
    }
}

fn failure_kind(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Unavailable => "モデルを利用できません",
        FailureKind::TimedOut => "応答がタイムアウトしました",
        FailureKind::Cancelled => "応答を中止しました",
        FailureKind::Refused => "応答できませんでした",
        FailureKind::Malformed => "応答を読み取れませんでした",
        FailureKind::Failed => "モデル応答に失敗しました",
        FailureKind::EmptyBody => "空の応答でした",
    }
}
