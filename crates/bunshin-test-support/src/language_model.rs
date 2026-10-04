use bunshin_core::{
    Availability, CancelFlag, LanguageModel, ModelAnswer, ModelError, ModelRequest, Tuning,
};
use std::{
    collections::VecDeque,
    sync::{Mutex, PoisonError},
};

/// Scripted calls consume one queued result and record their input, except when
/// already cancelled. An exhausted script returns `Failed`.
#[derive(Debug)]
pub struct ScriptedLanguageModel {
    availability: Result<Availability, ModelError>,
    answers: Mutex<VecDeque<Result<ModelAnswer, ModelError>>>,
    requests: Mutex<Vec<ModelRequest>>,
    cancel_gate: Option<std::sync::mpsc::Sender<()>>,
    probe_cancel_gate: Option<std::sync::mpsc::Sender<()>>,
}
impl ScriptedLanguageModel {
    /// An available model with the supplied sequence of results.
    pub fn new(answers: impl IntoIterator<Item = Result<ModelAnswer, ModelError>>) -> Self {
        Self {
            availability: Ok(Availability::Available),
            answers: Mutex::new(answers.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
            cancel_gate: None,
            probe_cancel_gate: None,
        }
    }
    /// Set the probe result independently of the answer script.
    #[must_use]
    pub fn with_availability(mut self, availability: Result<Availability, ModelError>) -> Self {
        self.availability = availability;
        self
    }
    /// Pause a cancellable availability probe until cancellation, signalling entry
    /// through the receiver so shutdown tests need no time-based wait.
    #[must_use]
    pub fn with_probe_cancel_gate(mut self) -> (Self, std::sync::mpsc::Receiver<()>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.probe_cancel_gate = Some(sender);
        (self, receiver)
    }
    /// Calls recorded in order, excluding calls cancelled before entry.
    #[must_use]
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
    /// Pause an available response until its cancellation flag is set. The returned
    /// receiver signals entry, allowing worker tests to synchronize without sleeping.
    #[must_use]
    pub fn with_cancel_gate(mut self) -> (Self, std::sync::mpsc::Receiver<()>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.cancel_gate = Some(sender);
        (self, receiver)
    }
}
impl LanguageModel for ScriptedLanguageModel {
    fn availability(&self) -> Result<Availability, ModelError> {
        self.availability
    }
    fn availability_with_cancel(&self, cancel: &CancelFlag) -> Result<Availability, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        if let Some(started) = &self.probe_cancel_gate {
            if started.send(()).is_err() {
                return Err(ModelError::Failed);
            }
            while !cancel.is_cancelled() {
                std::thread::yield_now();
            }
            return Err(ModelError::Cancelled);
        }
        self.availability()
    }
    fn respond(
        &self,
        request: &ModelRequest,
        cancel: &CancelFlag,
    ) -> Result<ModelAnswer, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        match self.availability()? {
            Availability::Available => {}
            Availability::Unavailable(reason) => return Err(ModelError::Unavailable(reason)),
        }
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        if let Some(started) = &self.cancel_gate {
            // A dropped observer must not leave a test worker waiting forever.
            if started.send(()).is_err() {
                return Err(ModelError::Failed);
            }
            while !cancel.is_cancelled() {
                std::thread::yield_now();
            }
            return Err(ModelError::Cancelled);
        }
        self.answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or(Err(ModelError::Failed))
    }
}
/// Every model honours cancellation before entry. An available fixture must answer
/// a harmless JSON-object request; unavailable fixtures return the matching kind.
///
/// # Panics
/// When an implementation violates these promises.
pub fn language_model_contract(mut make: impl FnMut() -> Box<dyn LanguageModel>) {
    let request = ModelRequest::new(
        "Reply with a JSON object.",
        "Return an empty JSON object.",
        r#"{"type":"object","title":"Empty","properties":{},"additionalProperties":false,"x-order":[],"required":[]}"#,
        Tuning::default(),
    );
    let model = make();
    let cancelled = CancelFlag::default();
    cancelled.cancel();
    assert_eq!(
        model.respond(&request, &cancelled),
        Err(ModelError::Cancelled)
    );
    assert_eq!(
        model.availability_with_cancel(&cancelled),
        Err(ModelError::Cancelled)
    );
    assert_eq!(
        model.availability_with_cancel(&CancelFlag::default()),
        model.availability()
    );
    match model.availability() {
        Ok(Availability::Available) => match model.respond(&request, &CancelFlag::default()) {
            Ok(answer) => assert_eq!(
                answer
                    .json
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>(),
                "{}",
                "the empty-object schema must produce an empty JSON object"
            ),
            Err(error) => panic!("contract response failed: {error:?}"),
        },
        Ok(Availability::Unavailable(reason)) => assert_eq!(
            model.respond(&request, &CancelFlag::default()),
            Err(ModelError::Unavailable(reason))
        ),
        Err(error) => panic!("contract probe failed: {error:?}"),
    }
}
