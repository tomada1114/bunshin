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
