//! Help descriptors borrow the dispatch table; the binary supplies all wording.
use super::keys::{KEY_TABLE, KeyBinding, KeyRegion, ScreenAction};

/// All supported key rows in product-table order, for the help overlay.
#[must_use]
pub const fn help_rows() -> &'static [KeyBinding] {
    KEY_TABLE
}

/// The task-pane line includes base-screen focus keys and starts with Help for narrow terminals.
#[must_use]
pub fn task_help() -> Vec<&'static KeyBinding> {
    KEY_TABLE
        .iter()
        .filter(|binding| {
            binding.region == KeyRegion::Tasks && binding.action == ScreenAction::Help
        })
        .chain(KEY_TABLE.iter().filter(|binding| {
            matches!(binding.region, KeyRegion::Tasks | KeyRegion::Main)
                && binding.action != ScreenAction::Help
        }))
        .collect()
}

/// The task-pane line while the cursor is in the leftovers block: Help first, then the
/// block's keys, then moving focus.
#[must_use]
pub fn leftovers_help() -> Vec<&'static KeyBinding> {
    KEY_TABLE
        .iter()
        .filter(|binding| {
            binding.region == KeyRegion::Tasks && binding.action == ScreenAction::Help
        })
        .chain(
            KEY_TABLE
                .iter()
                .filter(|binding| binding.region == KeyRegion::Leftovers),
        )
        .chain(KEY_TABLE.iter().filter(|binding| {
            binding.region == KeyRegion::Main && binding.action == ScreenAction::MoveFocus
        }))
        .collect()
}

/// The inbox overlay's own key line, in the product table's order; selection keys are
/// left to the help overlay.
#[must_use]
pub fn inbox_help() -> Vec<&'static KeyBinding> {
    KEY_TABLE
        .iter()
        .filter(|binding| {
            binding.region == KeyRegion::Inbox
                && !matches!(binding.action, ScreenAction::Previous | ScreenAction::Next)
        })
        .collect()
}
