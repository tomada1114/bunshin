//! Screen wording, separate from terminal drawing and core transitions.
use bunshin_core::{
    Now,
    day::{Day, DayError, TaskKind, TaskStatus, TaskView, store::DayStoreError},
    screen::keys::{ScreenAction, ScreenKey},
};

pub const APP_NAME: &str = "Bunshin";
pub const TASKS_TITLE: &str = " 今日のタスク ";
pub const CHAT_TITLE: &str = " 会話 ";
pub const INPUT_TITLE: &str = " 入力 ";
pub const EMPTY_TASKS: &str = "今日のタスクはありません";
pub const TOO_SMALL: &str = "端末が小さすぎます";
pub const EXPAND_TERMINAL: &str = "60×18 以上に広げてください";
pub const WAITING: &str = "待機中";
pub const MODEL_UNAVAILABLE: &str = "モデル: 使えません";
pub const THINKING: &str = "考え中…";
pub const THINKING_ROW: &str = "…";
pub const LONG_WAIT: &str = "まだ考えています（Esc で中止）";
pub const OWNER: &str = "あなた";
pub const SYSTEM: &str = "システム";
pub const CANCELLED_MARK: &str = "（中止）";
pub const QUIT_UNSAVED: &str = "保存できていない変更があります。終了しますか？（y で終了）";
pub const QUIT_UNCONFIRMED: &str = "保存済み・耐久性未確認です。終了しますか？（y で終了）";
pub const HELP_TITLE: &str = " キー操作（Esc で閉じる） ";
pub const HELP_IME: &str = "キーは英数入力（全角英数も使えます）";
pub const HELP_MARKS: &str = "表示: [ ] 未完了  [x] 完了  [-] やめた";

pub fn input_count(chars: usize, limit: usize) -> String {
    format!(" {chars}/{limit} ")
}
pub fn date_and_clock(day: &Day, now: Now) -> String {
    format!(
        "{}/{} {:02}:{:02}",
        day.date().month(),
        day.date().day(),
        now.local.hour(),
        now.local.minute()
    )
}
pub fn task_count(open: usize, total: usize) -> String {
    format!(" 未完了 {open}/{total} ")
}
pub fn status_mark(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "[ ]",
        TaskStatus::Done => "[x]",
        TaskStatus::Dropped => "[-]",
        TaskStatus::CarriedOver => "[>]",
    }
}
pub fn task_time(task: &TaskView) -> String {
    task.time.map_or_else(String::new, |time| {
        let prefix = match task.kind {
            TaskKind::Deadline => "〜",
            TaskKind::Appointment | TaskKind::Untimed => "",
        };
        format!("{prefix}{:02}:{:02}", time.hour(), time.minute())
    })
}
pub fn day_error(error: DayError) -> &'static str {
    match error {
        DayError::EmptyTitle | DayError::TitleTooLong => "タイトルは1〜80字で入力してください",
        DayError::LimitReached => "1日に持てるタスクは50件までです。",
        DayError::MissingTime => "締切と予定には時刻が必要です",
        DayError::UnexpectedTime => "時刻なしのタスクには時刻を指定できません。",
        DayError::InvalidTime => "時刻は 00:00〜23:59 で入力してください",
        DayError::TaskNotFound => "そのタスクはありません。",
        DayError::InvalidStatus => "その状態のタスクにはこの操作を使えません。",
        DayError::NothingToUndo => "取り消せる変更がありません。",
        DayError::NumberExhausted => "タスク番号をこれ以上増やせません。",
    }
}
pub fn action_label(action: ScreenAction) -> &'static str {
    match action {
        ScreenAction::Quit => "終了",
        ScreenAction::Undo => "取り消し",
        ScreenAction::MoveFocus => "入力欄⇔タスク欄",
        ScreenAction::Input => "入力欄へ",
        ScreenAction::Previous => "前を選択",
        ScreenAction::Next => "次を選択",
        ScreenAction::Help => "キー操作",
        ScreenAction::CloseHelp => "閉じる",
        ScreenAction::ChatUp => "前頁",
        ScreenAction::ChatDown => "次頁",
        ScreenAction::ChatLatest => "最新へ",
        ScreenAction::SendInput => "送信",
        ScreenAction::CancelInput => "中止／クリア",
    }
}
pub fn key_label(key: ScreenKey) -> String {
    match key {
        ScreenKey::Char(' ') => "Space".into(),
        ScreenKey::Char(c) => c.to_string(),
        ScreenKey::Up => "↑".into(),
        ScreenKey::Down => "↓".into(),
        ScreenKey::Left => "←".into(),
        ScreenKey::Right => "→".into(),
        ScreenKey::Enter => "Enter".into(),
        ScreenKey::Tab => "Tab".into(),
        ScreenKey::BackTab => "Shift+Tab".into(),
        ScreenKey::Esc => "Esc".into(),
        ScreenKey::Interrupt => "Ctrl+C".into(),
        ScreenKey::Undo => "Ctrl+Z".into(),
        ScreenKey::Backspace => "Backspace".into(),
        ScreenKey::Delete => "Delete".into(),
        ScreenKey::Home => "Home".into(),
        ScreenKey::End => "End".into(),
        ScreenKey::PageUp => "PgUp".into(),
        ScreenKey::PageDown => "PgDn".into(),
    }
}
pub fn startup_error(error: DayStoreError, date: &str) -> String {
    let root = if cfg!(target_os = "macos") {
        "~/Library/Application Support/io.github.tomada1114.bunshin"
    } else {
        "$XDG_DATA_HOME/bunshin（未設定時は ~/.local/share/bunshin）"
    };
    match error {
        DayStoreError::AlreadyLocked { pid: Some(pid) } => {
            format!("bunshin tui はすでに起動しています（参考 PID {pid}）。")
        }
        DayStoreError::AlreadyLocked { pid: None } => {
            "bunshin tui はすでに起動しています（参考 PID は不明です）。".into()
        }
        DayStoreError::Unavailable => "日別データの保存先を読み書きできませんでした。".into(),
        DayStoreError::PublishedButNotDurable => {
            "新しいデータは保存済みですが、耐久性は未確認です。".into()
        }
        DayStoreError::Unreadable => format!(
            "{root}/days/{date}.json を読めませんでした（データが壊れています）。ファイルはそのままです。"
        ),
        DayStoreError::NewerFormat { found } => format!(
            "{root}/days/{date}.json を読めませんでした（新しい形式 {found} です）。ファイルはそのままです。"
        ),
        DayStoreError::UnsupportedFormat { found } => format!(
            "{root}/days/{date}.json を読めませんでした（形式 {found} には対応していません）。ファイルはそのままです。"
        ),
    }
}
pub fn save_failure(error: DayStoreError) -> String {
    let reason = match error {
        DayStoreError::PublishedButNotDurable => return "新しいデータは保存済みですが、耐久性は未確認です。次の変更のときにもう一度保存します。".into(),
        DayStoreError::Unavailable => "保存先を読み書きできません",
        DayStoreError::Unreadable => "保存先のデータを読めません。元のファイルはそのままです",
        DayStoreError::NewerFormat { found: _ } => "保存先は新しい形式です。元のファイルはそのままです",
        DayStoreError::UnsupportedFormat { found: _ } => "保存先は対応していない形式です。元のファイルはそのままです",
        DayStoreError::AlreadyLocked { pid: _ } => "別の分身が起動しています",
    };
    format!(
        "保存できませんでした（{reason}）。変更は画面に残っていて、次の変更のときにもう一度保存します。"
    )
}
