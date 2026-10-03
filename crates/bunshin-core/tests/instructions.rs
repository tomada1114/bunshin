//! Instruction selection, boundaries, editor precedence, and the shared source contract.
use bunshin_core::{
    Tuning,
    instructions::{InstructionsError, InstructionsOrigin, InstructionsState, preferred_editor},
    prompt::rules::DEFAULT_INSTRUCTIONS,
};
use std::path::PathBuf;

#[test]
fn instructions_use_owner_text_at_six_hundred_unicode_characters_and_default_at_six_hundred_one() {
    for (count, origin) in [
        (599, InstructionsOrigin::Owner),
        (600, InstructionsOrigin::Owner),
        (601, InstructionsOrigin::TooLong),
    ] {
        let text = "文".repeat(count);
        let state = InstructionsState::resolve(
            Some(&text),
            PathBuf::from("/virtual/instructions.md"),
            Tuning::default(),
        );
        assert_eq!(state.origin, origin);
        assert_eq!(state.file_chars, Some(count));
        assert_eq!(state.limit, 600);
        assert_eq!(
            state.text,
            if count <= 600 {
                text
            } else {
                DEFAULT_INSTRUCTIONS.into()
            }
        );
    }
}

#[test]
fn missing_and_empty_instructions_have_distinct_default_reasons() {
    for (text, origin, count) in [
        (None, InstructionsOrigin::Missing, None),
        (Some(""), InstructionsOrigin::Empty, Some(0)),
        (Some(" \n\t　"), InstructionsOrigin::Empty, Some(4)),
    ] {
        let state = InstructionsState::resolve(
            text,
            PathBuf::from("/virtual/instructions.md"),
            Tuning::default(),
        );
        assert_eq!(state.origin, origin);
        assert_eq!(state.file_chars, count);
        assert_eq!(state.text, DEFAULT_INSTRUCTIONS);
        assert_eq!(state.path, PathBuf::from("/virtual/instructions.md"));
    }
}

#[test]
fn owner_instructions_are_used_whole_and_bounds_are_tunable() {
    let text = " 日本語\n";
    let state = InstructionsState::resolve(Some(text), PathBuf::new(), Tuning::default());
    assert_eq!(state.text, text);
    assert_eq!(state.file_chars, Some(5));
    let tuning = Tuning {
        instructions_max_chars: 4,
        ..Tuning::default()
    };
    assert_eq!(
        InstructionsState::resolve(Some(text), PathBuf::new(), tuning).origin,
        InstructionsOrigin::TooLong
    );
}

#[test]
fn edited_instructions_report_written_length_or_a_typed_failure() {
    let path = PathBuf::from("/virtual/instructions.md");
    for (text, result) in [
        (Some("文".repeat(600)), Ok(600)),
        (
            Some("文".repeat(601)),
            Err(InstructionsError::TooLong {
                chars: 601,
                limit: 600,
            }),
        ),
        (Some(" \n".into()), Ok(2)),
        (None, Err(InstructionsError::MissingAfterEdit)),
    ] {
        let state = InstructionsState::resolve(text.as_deref(), path.clone(), Tuning::default());
        assert_eq!(state.edited_length(), result);
    }
}

#[test]
fn visual_precedes_editor_and_empty_values_are_ignored() {
    for (visual, editor, expected) in [
        (Some("code --wait"), Some("vi"), Some("code --wait")),
        (Some(""), Some("vi"), Some("vi")),
        (Some(" \t"), Some("vi"), Some("vi")),
        (None, Some(""), None),
        (None, None, None),
    ] {
        assert_eq!(preferred_editor(visual, editor).as_deref(), expected);
    }
}

#[test]
fn in_memory_instructions_meet_the_shared_contract() {
    bunshin_test_support::instructions_contract(|| {
        Box::new(bunshin_test_support::InMemoryInstructions::new(
            PathBuf::from("/virtual/instructions.md"),
            None,
        ))
    });
}

#[test]
fn each_instructions_read_observes_owner_changes_and_initialization_preserves_them() {
    use bunshin_core::instructions::prepare_edit;
    use bunshin_test_support::InMemoryInstructions;
    let source = InMemoryInstructions::new(PathBuf::from("/virtual/instructions.md"), None);
    assert_eq!(
        InstructionsState::read(&source, Tuning::default())
            .expect("missing")
            .origin,
        InstructionsOrigin::Missing
    );
    prepare_edit(&source).expect("initialize");
    assert_eq!(
        bunshin_test_support::instructions_text(&source),
        Ok(Some(DEFAULT_INSTRUCTIONS.into()))
    );
    source.replace_text(Some("owner edit".into()));
    prepare_edit(&source).expect("preserve");
    assert_eq!(
        InstructionsState::read(&source, Tuning::default())
            .expect("reread")
            .text,
        "owner edit"
    );
    source.replace_text(Some("文".repeat(601)));
    assert_eq!(
        InstructionsState::read(&source, Tuning::default())
            .expect("over limit")
            .origin,
        InstructionsOrigin::TooLong
    );
    for error in [
        InstructionsError::Unavailable,
        InstructionsError::Unreadable,
    ] {
        let failing = InMemoryInstructions::new(PathBuf::new(), None).with_error(error);
        assert_eq!(
            InstructionsState::read(&failing, Tuning::default()),
            Err(error)
        );
        assert_eq!(prepare_edit(&failing), Err(error));
    }
}
