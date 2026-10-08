//! One clock reading supplies an instant and its local civil date-time.

use jiff::civil::DateTime;
use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch. Core never reads the clock itself (see [`Clock`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnixMillis(pub i64);

/// Both views of one wall-clock read, so a zone change cannot distort an elapsed gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Now {
    /// Milliseconds since the Unix epoch, used for gaps and timeouts.
    pub instant: UnixMillis,
    /// The same instant in the local zone, used for displayed wall times.
    pub local: DateTime,
}
impl Now {
    /// Convert another instant using this sample's fixed UTC offset. The system
    /// clock overrides this approximation with its zone's historical rules.
    #[must_use]
    pub fn at_fixed_offset(self, instant: UnixMillis) -> Option<Self> {
        let delta = instant.0.checked_sub(self.instant.0)?;
        let local = self
            .local
            .checked_add(jiff::SignedDuration::from_millis(delta))
            .ok()?;
        Some(Self { instant, local })
    }
}

/// The source of the current time: `SystemClock` (platform) in the app, `FixedClock`
/// (test-support) in tests. Injected so a test never sleeps and never depends on the date.
/// It is a wall clock: a later read may return an earlier time when the system time is
/// corrected, so nothing may depend on two reads being in order.
pub trait Clock: Send + Sync {
    /// Both views of the same sampled instant, at millisecond precision. The local
    /// view differs by a valid UTC offset; its subsecond part agrees with the instant.
    /// A zone change can move the local view without moving the instant.
    fn now(&self) -> Now;
    /// Local display time for a stored instant, without advancing the clock.
    /// The default keeps the current sample's offset; zone-aware adapters
    /// override it to account for historical daylight-saving transitions.
    fn local_at(&self, instant: UnixMillis) -> Option<Now> {
        self.now().at_fixed_offset(instant)
    }
}
