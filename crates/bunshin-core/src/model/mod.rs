//! Synchronous on-device model boundary, without processes or platform I/O in core.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use thiserror::Error;

/// A one-way cancellation signal shared by the caller and adapter.
#[derive(Clone, Debug, Default)]
pub struct CancelFlag(Arc<AtomicBool>);
impl CancelFlag {
    /// Cancel this call and every clone of this flag.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Input for one fresh model session. Text must never be logged.
#[derive(Clone, PartialEq, Eq)]
pub struct ModelRequest {
    /// Operating instructions, passed literally to the model.
    pub instructions: String,
    /// Prompt, passed unchanged through stdin rather than argv.
    pub prompt: String,
    /// JSON schema text; parsing belongs to the adapter.
    pub schema: String,
    /// Maximum wait measured by the adapter's monotonic clock.
    pub timeout: Duration,
}
impl ModelRequest {
    /// Build a fresh request using the same timeout tuning as the model adapter.
    /// The supplied text is copied unchanged; individual callers may then override
    /// the public timeout for a particular request.
    #[must_use]
    pub fn new(instructions: &str, prompt: &str, schema: &str, tuning: crate::Tuning) -> Self {
        Self {
            instructions: instructions.into(),
            prompt: prompt.into(),
            schema: schema.into(),
            timeout: tuning.model_timeout,
        }
    }
}
impl std::fmt::Debug for ModelRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelRequest")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}
/// JSON answer text, before core parses it into a proposal.
#[derive(Clone, PartialEq, Eq)]
pub struct ModelAnswer {
    /// Valid JSON text; no schema or domain validation is promised here.
    pub json: String,
}
impl std::fmt::Debug for ModelAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelAnswer").finish_non_exhaustive()
    }
}
/// An actionable reason the model cannot be reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnavailableReason {
    /// This operating system has no supported model adapter.
    UnsupportedOs,
    /// The model command is absent.
    NotInstalled,
    /// The owner must accept Apple's terms.
    TermsNotAccepted,
    /// The on-device model is still preparing.
    ModelNotReady,
    /// The model is unavailable for another or unrecognized reason.
    Other,
}
/// Result of probing the model command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// A call can be attempted.
    Available,
    /// The owner can act on this reason.
    Unavailable(UnavailableReason),
}
/// Failures contain kinds only, never model input, output, or OS diagnostic text.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    /// The model cannot be reached.
    #[error("model unavailable: {0:?}")]
    Unavailable(UnavailableReason),
    /// The request's deadline elapsed.
    #[error("model timed out")]
    TimedOut,
    /// The caller cancelled the request.
    #[error("model cancelled")]
    Cancelled,
    /// The model refused this request.
    #[error("model refused")]
    Refused,
    /// The successful process returned invalid JSON.
    #[error("malformed model answer")]
    Malformed,
    /// An unclassified process or I/O failure.
    #[error("model failed")]
    Failed,
}
/// One synchronous call per session; implementations must stop and reap any child
/// before returning on cancellation or timeout. A pre-cancelled call spawns nothing.
pub trait LanguageModel: Send + Sync {
    /// Probe readiness without exposing command diagnostics.
    ///
    /// # Errors
    /// An unclassified probe failure, including its timeout.
    fn availability(&self) -> Result<Availability, ModelError>;
    /// Probe with cancellation, without spawning work when already cancelled.
    /// Process-backed adapters override this to stop and reap a running probe.
    /// The compatibility default checks cancellation before and after `availability`;
    /// it cannot interrupt an implementation's blocking probe.
    ///
    /// # Errors
    /// Cancellation, or the same failures as `availability`.
    fn availability_with_cancel(&self, cancel: &CancelFlag) -> Result<Availability, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        let result = self.availability();
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        result
    }
    /// Return JSON text, or a typed failure. Prompt bytes are preserved.
    ///
    /// # Errors
    /// Unavailability, timeout, cancellation, refusal, malformed JSON, or I/O failure.
    fn respond(
        &self,
        request: &ModelRequest,
        cancel: &CancelFlag,
    ) -> Result<ModelAnswer, ModelError>;
}
