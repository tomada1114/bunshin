//! The app's tunables, shared by every feature.
use std::time::Duration;

use crate::board::BoardTuning;

const DEFAULT_MODEL_TIMEOUT: Duration = Duration::from_secs(30);

/// Shared domain tunables, adjustable without reading configuration in core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tuning {
    /// Board scheduling and display bounds.
    pub board: BoardTuning,
    /// Worker polling and in-progress status delay.
    pub chat: ChatTuning,
    /// Maximum model call and availability-probe wait; callers may tune it.
    pub model_timeout: Duration,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            board: BoardTuning::default(),
            chat: ChatTuning::default(),
            model_timeout: DEFAULT_MODEL_TIMEOUT,
        }
    }
}

/// Board status and worker polling delays; time is supplied by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatTuning {
    /// Terminal polling bound for worker completions.
    pub poll_interval: Duration,
    /// Delay before the board header shows an in-progress turn.
    pub thinking_after: Duration,
}
impl Default for ChatTuning {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            thinking_after: Duration::from_millis(300),
        }
    }
}
