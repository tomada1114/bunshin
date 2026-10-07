//! Every sentence the screen presents to its owner.
mod tui;
pub use tui::*;
mod chat;
pub use chat::*;

pub const ABOUT: &str = "ターミナルの分身";
pub const TUI_ABOUT: &str = "全画面を開く（対話型ターミナルが必要です）";
pub const HOME_MISSING: &str = "HOME が設定されていないため、保存先が分かりません。";
pub const LOGGING_UNAVAILABLE: &str = "この実行ではログを保存できません。";
pub const TERMINAL_MISSING: &str = "bunshin tui は端末の中で実行してください";
pub const TERMINAL_FAILED: &str = "端末を使えませんでした。";
