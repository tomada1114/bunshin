//! Language-model boundary contracts and regression cases.

use bunshin_core::{CancelFlag, LanguageModel, ModelError, ModelRequest};
use bunshin_test_support::{ScriptedLanguageModel, language_model_contract};
use std::time::Duration;

#[test]
fn cancellation_does_not_consume_script() {
    let model = ScriptedLanguageModel::new([Err(ModelError::Refused)]);
    let request = ModelRequest {
        instructions: "rules".into(),
        prompt: "hello".into(),
        schema: "{}".into(),
        timeout: Duration::from_secs(1),
    };
    let cancel = CancelFlag::default();
    cancel.cancel();
    assert_eq!(model.respond(&request, &cancel), Err(ModelError::Cancelled));
    assert_eq!(
        model.respond(&request, &CancelFlag::default()),
        Err(ModelError::Refused)
    );
    assert_eq!(model.requests(), vec![request]);
    assert_eq!(
        model.respond(&model.requests()[0], &CancelFlag::default()),
        Err(ModelError::Failed)
    );
}

#[test]
fn scripted_model_replays_each_typed_error_and_success_in_order() {
    use bunshin_core::{ModelAnswer, UnavailableReason};
    let errors = [
        ModelError::Unavailable(UnavailableReason::NotInstalled),
        ModelError::Unavailable(UnavailableReason::TermsNotAccepted),
        ModelError::Unavailable(UnavailableReason::UnsupportedOs),
        ModelError::TimedOut,
        ModelError::Cancelled,
        ModelError::Refused,
        ModelError::Malformed,
        ModelError::Failed,
    ];
    let answer = ModelAnswer { json: "{}".into() };
    let model = ScriptedLanguageModel::new(errors.into_iter().map(Err).chain([Ok(answer.clone())]));
    let request = ModelRequest {
        instructions: "private rules".into(),
        prompt: "private task".into(),
        schema: "{}".into(),
        timeout: Duration::from_secs(1),
    };
    for error in errors {
        assert_eq!(model.respond(&request, &CancelFlag::default()), Err(error));
        assert!(!format!("{error}").contains("private"));
    }
    assert_eq!(model.respond(&request, &CancelFlag::default()), Ok(answer));
    assert_eq!(model.requests(), vec![request; 9]);
}

#[test]
fn unavailable_fake_preserves_script_and_records_no_request() {
    use bunshin_core::{Availability, UnavailableReason};
    let request = ModelRequest {
        instructions: String::new(),
        prompt: String::new(),
        schema: "{}".into(),
        timeout: Duration::ZERO,
    };
    for reason in [
        UnavailableReason::NotInstalled,
        UnavailableReason::TermsNotAccepted,
        UnavailableReason::UnsupportedOs,
    ] {
        let model =
            ScriptedLanguageModel::new([]).with_availability(Ok(Availability::Unavailable(reason)));
        assert_eq!(model.availability(), Ok(Availability::Unavailable(reason)));
        assert_eq!(
            model.respond(&request, &CancelFlag::default()),
            Err(ModelError::Unavailable(reason))
        );
        assert_eq!(model.requests(), vec![]);
        language_model_contract(|| {
            Box::new(
                ScriptedLanguageModel::new([])
                    .with_availability(Ok(Availability::Unavailable(reason))),
            )
        });
    }
    let failed = ScriptedLanguageModel::new([]).with_availability(Err(ModelError::Failed));
    assert_eq!(failed.availability(), Err(ModelError::Failed));
    assert_eq!(
        failed.respond(&request, &CancelFlag::default()),
        Err(ModelError::Failed)
    );
    assert_eq!(failed.requests(), vec![]);
}

#[test]
fn cloned_cancel_flag_is_shared_and_model_debug_redacts_text() {
    use bunshin_core::ModelAnswer;
    let original = CancelFlag::default();
    assert!(!original.is_cancelled());
    original.clone().cancel();
    assert!(original.is_cancelled());
    let request = ModelRequest {
        instructions: "secret-instructions".into(),
        prompt: "secret-prompt".into(),
        schema: "secret-schema".into(),
        timeout: Duration::ZERO,
    };
    let answer = ModelAnswer {
        json: "secret-answer".into(),
    };
    assert!(!format!("{request:?}").contains("secret"));
    assert!(!format!("{answer:?}").contains("secret"));
    let model = ScriptedLanguageModel::new([Ok(answer)]);
    assert!(!format!("{model:?}").contains("secret"));
}
