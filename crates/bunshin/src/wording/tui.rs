//! Board-screen wording, separate from terminal drawing and core transitions.
use bunshin_core::{
    ModelError, Now, UnavailableReason,
    board::FailureKind,
    screen::{BoardFailure, BoardStatus},
};

pub const APP_NAME: &str = "Bunshin";
pub const BOARD_TITLE: &str = " 掲示板 ";
pub const INPUT_TITLE: &str = " 入力 ";
pub const EMPTY_BOARD: &str = "まだ投稿はありません";
pub const TOO_SMALL: &str = "端末が小さすぎます";
pub const EXPAND_TERMINAL: &str = "60×18 以上に広げてください";
pub const IDLE: &str = "待機中";
pub const WRITING: &str = "書き込み中…";
pub const INPUT_HINT: &str = "Tab  掲示板へ    q  終了";

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
