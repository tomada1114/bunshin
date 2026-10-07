//! Help descriptors borrow the dispatch table; the binary supplies all wording.
use super::keys::{KEY_TABLE, KeyBinding, KeyRegion, ScreenAction};

/// All supported key rows in screen-table order.
#[must_use]
pub const fn help_rows() -> &'static [KeyBinding] {
    KEY_TABLE
}

/// The task-pane line starts with Help for narrow terminals.
#[must_use]
pub fn task_help() -> Vec<&'static KeyBinding> {
    KEY_TABLE
        .iter()
        .filter(|binding| {
            binding.region == KeyRegion::Tasks
                || (binding.region == KeyRegion::Main && binding.action == ScreenAction::MoveFocus)
                || binding.region == KeyRegion::Help
        })
        .collect()
}
