//! Terminal-independent state and text editing for the board screen.

mod board_screen;
pub use board_screen::{BoardFailure, BoardFocus, BoardRequest, BoardScreen, BoardStatus};
mod input;
pub use input::InputBuffer;
mod keys;
pub use keys::ScreenKey;
mod viewport;
