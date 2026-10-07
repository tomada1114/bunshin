//! Main screen wording, separate from terminal drawing and core transitions.
use bunshin_core::day::store::DayStoreError;
use bunshin_core::{
    Now,
    day::{Day, DayError, TaskKind, TaskStatus, TaskView},
    screen::{
        keys::{KeyRegion, ScreenAction, ScreenKey},
        task_form::FormError,
    },
};

pub const APP_NAME: &str = "Bunshin";
pub const TASKS_TITLE: &str = " 今日のタスク ";
pub const CHAT_TITLE: &str = " チャット ";
pub const INPUT_TITLE: &str = " 入力 ";
pub const EMPTY_TASKS: &str = "まだありません。Tab で移って a で追加";
pub const TOO_SMALL: &str = "端末が小さすぎます";
pub const EXPAND_TERMINAL: &str = "60×18 以上に広げてください";
pub const WAITING: &str = "待機中";
pub const NOT_SAVED: &str = "保存できません";
pub const NOT_DURABLE: &str = "保存済み・耐久性未確認";
pub const ERROR_SPEAKER: &str = "エラー";
pub const QUIT_UNSAVED: &str = "保存できていない変更があります。終了しますか？（y で終了）";
pub const QUIT_UNCONFIRMED: &str = "保存済み・耐久性未確認です。終了しますか？（y で終了）";
pub const ADD_TASK: &str = " タスクを追加 ";
pub const EDIT_TASK: &str = " タスクを編集 ";
pub const FORM_TITLE: &str = "タイトル";
pub const FORM_KIND: &str = "種類";
pub const FORM_TIME: &str = "時刻";
pub const HELP_TITLE: &str = " キー操作（? または Esc で閉じる） ";
pub const HELP_IME: &str = "キーは英数入力（全角英数も使えます）";
pub const HELP_MARKS: &str = "表示: [ ] 未完了  [x] 完了  [-] やめた  [>] 持ち越し済み";
pub const HELP_TIMES: &str = "10:00 予定（その時刻に始まる）  〜15:00 締切（その時刻までに）";
pub const HEADER_SEPARATOR: &str = " | ";
pub const CHECKING_IN: &str = "見回り中…";
pub const LEFTOVERS_NO_TASKS: &str = "（今日のタスクはまだありません）";
pub const INBOX_TITLE: &str = " 受信箱 ";
pub const INBOX_EMPTY: &str = "対応が必要なものはありません。";

pub fn header_inbox(count: usize) -> String {
    format!("受信箱 {count}")
}
pub fn header_held(count: usize) -> String {
    format!("保留 {count}")
}
pub fn header_muted(until: &str) -> String {
    format!("ミュート中 〜{until}")
}
/// Hours and minutes arrive as plain numbers so the binary names no time crate.
pub fn header_outside_hours(hour: i8, minute: i8) -> String {
    format!("時間外（{hour:02}:{minute:02} から）")
}
pub fn header_next_look(at: Option<(i8, i8)>) -> String {
    at.map_or_else(
        || "次の見回り —".into(),
        |(hour, minute)| format!("次の見回り {hour:02}:{minute:02}"),
    )
}
pub fn leftovers_heading(previous: &Day) -> String {
    format!(
        "前日の残り {} ─ 持ち越すか、やめるか決めてください",
        short_date(previous)
    )
}
pub fn inbox_count(open: usize) -> String {
    format!(" 未対応 {open} ")
}
pub fn inbox_note(minutes: u16) -> String {
    format!("── 未対応は今日の分だけ。{minutes}分反応がないと「無視」として秘書に伝わります")
}
fn short_date(day: &Day) -> String {
    const WEEKDAYS: [&str; 7] = ["日", "月", "火", "水", "木", "金", "土"];
    let date = day.date();
    let weekday = usize::try_from(date.weekday().to_sunday_zero_offset()).unwrap_or(0);
    format!("{}/{}({})", date.month(), date.day(), WEEKDAYS[weekday])
}

pub fn date_and_clock(day: &Day, now: Now) -> String {
    format!(
        "{} {:02}:{:02}",
        short_date(day),
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
pub fn kind_label(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Untimed => "時刻なし",
        TaskKind::Deadline => "締切",
        TaskKind::Appointment => "予定",
    }
}
pub fn form_error(error: FormError) -> &'static str {
    match error {
        FormError::TitleLength => "タイトルは1〜80字で入力してください",
        FormError::TimeRequired => "締切と予定には時刻が必要です",
        FormError::InvalidTime => "時刻は 00:00〜23:59 で入力してください",
    }
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
pub fn region_label(region: KeyRegion) -> &'static str {
    match region {
        KeyRegion::Anywhere => "どこでも",
        KeyRegion::Main => "メイン画面",
        KeyRegion::Tasks => "タスク欄",
        KeyRegion::Form => "フォーム",
        KeyRegion::Help => "ヘルプ",
        KeyRegion::Instructions => "指示文",
        KeyRegion::Input => "入力欄",
        KeyRegion::Leftovers => "前日の残り",
        KeyRegion::Inbox => "受信箱",
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
        ScreenAction::Done => "完了⇔未完了",
        ScreenAction::Drop => "やめる⇔戻す",
        ScreenAction::Add => "追加",
        ScreenAction::Edit | ScreenAction::EditText => "編集",
        ScreenAction::Delete => "削除",
        ScreenAction::Mute => "ミュート60分⇔解除",
        ScreenAction::Help => "全キー",
        ScreenAction::CloseHelp | ScreenAction::CloseInstructions | ScreenAction::InboxClose => {
            "閉じる"
        }
        ScreenAction::SaveForm => "保存",
        ScreenAction::NextField => "次へ",
        ScreenAction::PreviousField => "前へ",
        ScreenAction::Left => "左／種類",
        ScreenAction::Right => "右／種類",
        ScreenAction::CancelForm | ScreenAction::DropLeftover => "やめる",
        ScreenAction::Instructions => "指示文",
        ScreenAction::InstructionsUp => "上へ",
        ScreenAction::InstructionsDown => "下へ",
        ScreenAction::ChatUp => "前頁",
        ScreenAction::ChatDown => "次頁",
        ScreenAction::ChatLatest => "最新へ",
        ScreenAction::SendInput => "送信",
        ScreenAction::CancelInput => "中止／クリア",
        ScreenAction::CarryLeftover => "持ち越し",
        ScreenAction::CarryAllLeftovers => "全部持ち越し",
        ScreenAction::DropAllLeftovers => "全部やめる",
        ScreenAction::Inbox => "受信箱",
        ScreenAction::InboxRespond => "返事する／了解",
        ScreenAction::InboxAcknowledgeNotes => "お知らせを全部了解",
        ScreenAction::InboxTask => "タスクへ",
        ScreenAction::CloseInbox => "戻る",
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
        DayStoreError::AlreadyLocked { pid: Some(pid) } => format!(
            "bunshin tui はすでに起動しています（参考 PID {pid}）。PID は起動直後に前回の値を示す場合があります。"
        ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use bunshin_core::{Tuning, UnixMillis};
    #[test]
    fn save_failures_name_every_typed_cause_without_owner_data() {
        for (error, reason) in [
            (DayStoreError::Unavailable, "保存先を読み書きできません"),
            (
                DayStoreError::Unreadable,
                "保存先のデータを読めません。元のファイルはそのままです",
            ),
            (
                DayStoreError::NewerFormat { found: 2 },
                "保存先は新しい形式です。元のファイルはそのままです",
            ),
            (
                DayStoreError::UnsupportedFormat { found: 0 },
                "保存先は対応していない形式です。元のファイルはそのままです",
            ),
            (
                DayStoreError::AlreadyLocked { pid: Some(123) },
                "別の分身が起動しています",
            ),
        ] {
            assert_eq!(
                save_failure(error),
                format!(
                    "保存できませんでした（{reason}）。変更は画面に残っていて、次の変更のときにもう一度保存します。"
                )
            );
        }
        assert_eq!(
            save_failure(DayStoreError::PublishedButNotDurable),
            "新しいデータは保存済みですが、耐久性は未確認です。次の変更のときにもう一度保存します。"
        );
    }
    #[test]
    fn startup_errors_use_advisory_pid_and_symbolic_file_locations() {
        assert_eq!(
            startup_error(
                DayStoreError::AlreadyLocked { pid: Some(4821) },
                "2026-10-02"
            ),
            "bunshin tui はすでに起動しています（参考 PID 4821）。PID は起動直後に前回の値を示す場合があります。"
        );
        assert_eq!(
            startup_error(DayStoreError::AlreadyLocked { pid: None }, "2026-10-02"),
            "bunshin tui はすでに起動しています（参考 PID は不明です）。"
        );
        assert_eq!(
            startup_error(DayStoreError::Unavailable, "2026-10-02"),
            "日別データの保存先を読み書きできませんでした。"
        );
        assert_eq!(
            startup_error(DayStoreError::PublishedButNotDurable, "2026-10-02"),
            "新しいデータは保存済みですが、耐久性は未確認です。"
        );
        let root = if cfg!(target_os = "macos") {
            "~/Library/Application Support/io.github.tomada1114.bunshin"
        } else {
            "$XDG_DATA_HOME/bunshin（未設定時は ~/.local/share/bunshin）"
        };
        for (error, cause) in [
            (DayStoreError::Unreadable, "データが壊れています"),
            (DayStoreError::NewerFormat { found: 2 }, "新しい形式 2 です"),
            (
                DayStoreError::UnsupportedFormat { found: 0 },
                "形式 0 には対応していません",
            ),
        ] {
            assert_eq!(
                startup_error(error, "2026-10-02"),
                format!(
                    "{root}/days/2026-10-02.json を読めませんでした（{cause}）。ファイルはそのままです。"
                )
            );
        }
    }
    #[test]
    fn header_uses_the_loaded_logical_date_and_current_clock_before_four() {
        let day = Day::new("2026-10-02".parse().expect("date"), Tuning::default());
        let now = Now {
            instant: UnixMillis(0),
            local: "2026-10-03T01:30:00".parse().expect("local"),
        };
        assert_eq!(date_and_clock(&day, now), "10/2(金) 01:30");
    }
}
