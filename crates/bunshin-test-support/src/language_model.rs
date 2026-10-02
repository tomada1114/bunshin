use bunshin_core::{
    Availability, CancelFlag, LanguageModel, ModelAnswer, ModelError, ModelRequest,
};
use std::{
    collections::VecDeque,
    sync::{Mutex, PoisonError},
    time::Duration,
};

/// Scripted calls consume one queued result and record their input, except when
/// already cancelled. An exhausted script returns `Failed`.
#[derive(Debug)]
pub struct ScriptedLanguageModel {
    availability: Result<Availability, ModelError>,
    answers: Mutex<VecDeque<Result<ModelAnswer, ModelError>>>,
    requests: Mutex<Vec<ModelRequest>>,
}
impl ScriptedLanguageModel {
    /// An available model with the supplied sequence of results.
    pub fn new(answers: impl IntoIterator<Item = Result<ModelAnswer, ModelError>>) -> Self {
        Self {
            availability: Ok(Availability::Available),
            answers: Mutex::new(answers.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
    /// Set the probe result independently of the answer script.
    #[must_use]
    pub fn with_availability(mut self, availability: Result<Availability, ModelError>) -> Self {
        self.availability = availability;
        self
    }
    /// Calls recorded in order, excluding calls cancelled before entry.
    #[must_use]
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}
impl LanguageModel for ScriptedLanguageModel {
    fn availability(&self) -> Result<Availability, ModelError> {
        self.availability
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
    let request = ModelRequest {
        instructions: "Reply with a JSON object.".into(),
        prompt: "Return an empty JSON object.".into(),
        schema: r#"{"type":"object","title":"Empty","properties":{},"additionalProperties":false,"x-order":[],"required":[]}"#.into(),
        timeout: Duration::from_secs(30),
    };
    let model = make();
    let cancelled = CancelFlag::default();
    cancelled.cancel();
    assert_eq!(
        model.respond(&request, &cancelled),
        Err(ModelError::Cancelled)
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
