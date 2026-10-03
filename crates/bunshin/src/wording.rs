//! Every sentence the shell presents to its owner.

pub const ABOUT: &str = "ターミナルの秘書";
pub const TUI_ABOUT: &str = "全画面を開く（対話型ターミナルが必要です）";
pub const SHELL_TITLE: &str = "分身";
pub const SHELL_EMPTY: &str = "今日のタスクはありません";
pub const QUIT: &str = "終了";
pub const HOME_MISSING: &str = "HOME is not set, so the log directory cannot be found";
pub const LOGGING_UNAVAILABLE: &str = "logging is unavailable for this run";
pub const TERMINAL_MISSING: &str =
    "tui needs an interactive terminal on standard input and standard output";
pub const TERMINAL_FAILED: &str = "the terminal could not be used";

use bunshin_core::instructions::{
    EditorError, InstructionsError, InstructionsLocation, InstructionsOrigin, InstructionsState,
};

pub const INSTRUCTIONS_ABOUT: &str = "使っている指示文とファイルの場所を表示する";
pub const INSTRUCTIONS_EDIT_ABOUT: &str = "指示文を自分のエディタで編集する";
pub const INSTRUCTIONS_HOME_MISSING: &str =
    "HOME が設定されていないため、指示文ファイルの場所が分かりません。";
pub const STDOUT_UNAVAILABLE: &str = "標準出力に書き込めませんでした。";

pub fn instructions_source(state: &InstructionsState) -> String {
    let path = state.path.display();
    let chars = state.file_chars.unwrap_or_default();
    let limit = state.limit;
    match state.origin {
        InstructionsOrigin::Owner => format!("ファイル: {path}（{chars}/{limit}字）"),
        InstructionsOrigin::Missing => {
            format!("既定の指示文を使っています（ファイルがありません）: {path}")
        }
        InstructionsOrigin::Empty => {
            format!("既定の指示文を使っています（ファイルが空です（{chars}/{limit}字））: {path}")
        }
        InstructionsOrigin::TooLong => format!(
            "既定の指示文を使っています（ファイルの指示文が{limit}字を超えています（{chars}字））: {path}"
        ),
    }
}
pub fn no_editor(location: InstructionsLocation) -> String {
    let path = match location {
        InstructionsLocation::MacosHome => format!(
            "~/Library/Application Support/{}/instructions.md",
            bunshin_platform::BUNDLE_IDENTIFIER
        ),
        InstructionsLocation::LinuxHome => format!(
            "~/.local/share/{}/instructions.md",
            bunshin_platform::XDG_APP_NAME
        ),
        InstructionsLocation::XdgDataHome => format!(
            "$XDG_DATA_HOME/{}/instructions.md",
            bunshin_platform::XDG_APP_NAME
        ),
    };
    format!(
        "エディタが設定されていません。VISUAL か EDITOR を設定するか、次のファイルを直接編集してください: {path}"
    )
}
pub fn instructions_saved(chars: usize, limit: usize) -> String {
    format!("指示文を保存しました（{chars}/{limit}字）。次の呼び出しから使われます。")
}
pub fn instructions_error(error: InstructionsError) -> String {
    match error {
        InstructionsError::UnsafeEntry => "指示文にはリンクや通常のファイル以外のものは使えません。通常のファイルに置き換えてください。ファイルはそのままです。".into(),
        InstructionsError::Permissions => "指示文の権限が必要な設定ではありません。保存先を0700、ファイルを0600にしてください。ファイルはそのままです。".into(),
        InstructionsError::Unavailable => "指示文ファイルを読み書きできませんでした。".into(),
        InstructionsError::Unreadable => {
            "指示文をUTF-8として読めませんでした。ファイルはそのままです。".into()
        }
        InstructionsError::MissingAfterEdit => {
            "編集後の指示文ファイルがありません。既定の指示文が使われます。".into()
        }
        InstructionsError::TooLong { chars, limit } => format!(
            "指示文が{limit}字を超えています（{chars}字）。直すまでは既定の指示文が使われます。"
        ),
    }
}
pub fn editor_error(error: EditorError) -> String {
    match error {
        EditorError::Unavailable => {
            "エディタを起動できませんでした。指示文ファイルはそのままです。".into()
        }
        EditorError::Exited { code: Some(code) } => {
            format!("エディタが終了コード{code}で終了しました。指示文ファイルを確認してください。")
        }
        EditorError::Exited { code: None } => {
            "エディタが中断されました。指示文ファイルを確認してください。".into()
        }
    }
}

/// Help for the read-only task list.
pub const TODAY_ABOUT: &str = "今日のタスクを読み取って表示する";
/// Help for the stable public JSON view.
pub const TODAY_JSON_HELP: &str = "形式バージョン付きの JSON を表示する";
/// Missing HOME must not guess where to read owner data.
pub const TODAY_HOME_MISSING: &str =
    "HOME が設定されていないため、今日のデータの場所が分かりません。";
/// One data-free Japanese line for each typed day-store failure.
pub fn today_error(error: bunshin_core::day::store::DayStoreError) -> &'static str {
    use bunshin_core::day::store::DayStoreError;
    match error {
        DayStoreError::Unavailable | DayStoreError::AlreadyLocked { pid: _ } => {
            "今日のデータを読めませんでした（保存先を確認してください）"
        }
        DayStoreError::PublishedButNotDurable => {
            "今日のデータを読めませんでした（保存の耐久性を確認できませんでした）"
        }
        DayStoreError::Unreadable => "今日のデータを読めませんでした（ファイルを確認してください）",
        DayStoreError::NewerFormat { found: _ } => {
            "今日のデータを読めませんでした（ファイルの形式が新しすぎます）"
        }
        DayStoreError::UnsupportedFormat { found: _ } => {
            "今日のデータを読めませんでした（ファイルの形式に対応していません）"
        }
    }
}
/// A full task row; control characters become spaces in plain terminal output.
pub fn today_row(task: &bunshin_core::day::today_view::TodayTask) -> String {
    use bunshin_core::day::{TaskKind, TaskStatus};
    let mark = match task.status {
        TaskStatus::Open => "[ ]",
        TaskStatus::Done => "[x]",
        TaskStatus::Dropped => "[-]",
        TaskStatus::CarriedOver => "[>]",
    };
    let time = task
        .time
        .map(|time| format!("{:02}:{:02}", time.hour(), time.minute()));
    let time = match (task.kind, time) {
        (TaskKind::Deadline, Some(time)) => format!("〜{time}"),
        (TaskKind::Appointment, Some(time)) => format!("{time}  "),
        (TaskKind::Untimed, Some(_))
        | (TaskKind::Untimed | TaskKind::Deadline | TaskKind::Appointment, None) => {
            "       ".into()
        }
    };
    let title = task
        .title
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    format!("{}  {mark}  {time}  {title}", task.number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_editor_locations_are_symbolic_and_owner_independent() {
        for (location, path) in [
            (
                InstructionsLocation::MacosHome,
                "~/Library/Application Support/io.github.tomada1114.bunshin/instructions.md",
            ),
            (
                InstructionsLocation::LinuxHome,
                "~/.local/share/bunshin/instructions.md",
            ),
            (
                InstructionsLocation::XdgDataHome,
                "$XDG_DATA_HOME/bunshin/instructions.md",
            ),
        ] {
            assert_eq!(
                no_editor(location),
                format!(
                    "エディタが設定されていません。VISUAL か EDITOR を設定するか、次のファイルを直接編集してください: {path}"
                )
            );
        }
    }

    #[test]
    fn instructions_failures_have_actionable_wording_without_owner_text() {
        for (error, expected) in [
            (
                InstructionsError::UnsafeEntry,
                "指示文にはリンクや通常のファイル以外のものは使えません。通常のファイルに置き換えてください。ファイルはそのままです。",
            ),
            (
                InstructionsError::Permissions,
                "指示文の権限が必要な設定ではありません。保存先を0700、ファイルを0600にしてください。ファイルはそのままです。",
            ),
            (
                InstructionsError::Unavailable,
                "指示文ファイルを読み書きできませんでした。",
            ),
            (
                InstructionsError::Unreadable,
                "指示文をUTF-8として読めませんでした。ファイルはそのままです。",
            ),
            (
                InstructionsError::MissingAfterEdit,
                "編集後の指示文ファイルがありません。既定の指示文が使われます。",
            ),
            (
                InstructionsError::TooLong {
                    chars: 612,
                    limit: 600,
                },
                "指示文が600字を超えています（612字）。直すまでは既定の指示文が使われます。",
            ),
        ] {
            assert_eq!(instructions_error(error), expected);
        }
        for (error, expected) in [
            (
                EditorError::Unavailable,
                "エディタを起動できませんでした。指示文ファイルはそのままです。",
            ),
            (
                EditorError::Exited { code: Some(7) },
                "エディタが終了コード7で終了しました。指示文ファイルを確認してください。",
            ),
            (
                EditorError::Exited { code: None },
                "エディタが中断されました。指示文ファイルを確認してください。",
            ),
        ] {
            assert_eq!(editor_error(error), expected);
        }
    }

    #[test]
    fn today_store_failures_are_exhaustive_data_free_japanese_lines() {
        use bunshin_core::day::store::DayStoreError;
        for (error, expected) in [
            (
                DayStoreError::Unavailable,
                "今日のデータを読めませんでした（保存先を確認してください）",
            ),
            (
                DayStoreError::PublishedButNotDurable,
                "今日のデータを読めませんでした（保存の耐久性を確認できませんでした）",
            ),
            (
                DayStoreError::Unreadable,
                "今日のデータを読めませんでした（ファイルを確認してください）",
            ),
            (
                DayStoreError::NewerFormat { found: 2 },
                "今日のデータを読めませんでした（ファイルの形式が新しすぎます）",
            ),
            (
                DayStoreError::UnsupportedFormat { found: 0 },
                "今日のデータを読めませんでした（ファイルの形式に対応していません）",
            ),
            (
                DayStoreError::AlreadyLocked { pid: Some(123) },
                "今日のデータを読めませんでした（保存先を確認してください）",
            ),
        ] {
            assert_eq!(today_error(error), expected);
        }
    }
}
