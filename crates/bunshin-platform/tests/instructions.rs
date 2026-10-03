//! Private UTF-8 instructions with reads that never create or truncate files.
use bunshin_core::instructions::{InstructionsError, InstructionsSource};
use bunshin_platform::{FileInstructions, instructions_file};
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn instructions_initialize_owner_only_and_never_overwrite_existing_bytes() {
    let scratch = tempfile::tempdir().expect("scratch");
    let root = scratch.path().join("data");
    let source = FileInstructions::new(root.clone());
    assert_eq!(source.path(), instructions_file(&root));
    assert_eq!(bunshin_test_support::instructions_text(&source), Ok(None));
    assert!(!root.exists());
    source.ensure_default("default").expect("initialize");
    assert_eq!(
        fs::metadata(&root).expect("root").permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(source.path())
            .expect("file")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    fs::write(source.path(), [0xff]).expect("invalid UTF-8");
    assert_eq!(
        bunshin_test_support::instructions_text(&source),
        Err(InstructionsError::Unreadable)
    );
    source
        .ensure_default("replacement")
        .expect("preserve existing");
    assert_eq!(fs::read(source.path()).expect("bytes preserved"), [0xff]);
}

#[test]
fn a_linked_instructions_root_cannot_read_or_change_external_files() {
    let scratch = tempfile::tempdir().expect("scratch");
    let external = scratch.path().join("external");
    fs::create_dir(&external).expect("external dir");
    let path = external.join("instructions.md");
    fs::write(&path, "external text").expect("external text");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("external permissions");
    let root = scratch.path().join("data");
    std::os::unix::fs::symlink(&external, &root).expect("linked root");
    let source = FileInstructions::new(root);
    assert_eq!(
        bunshin_test_support::instructions_text(&source),
        Err(InstructionsError::Unavailable)
    );
    assert_eq!(
        source.ensure_default("default"),
        Err(InstructionsError::Unavailable)
    );
    assert_eq!(
        source.protect_after_edit(),
        Err(InstructionsError::Unavailable)
    );
    assert_eq!(
        fs::read_to_string(&path).expect("unchanged"),
        "external text"
    );
    assert_eq!(
        fs::metadata(path)
            .expect("permissions")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
}

#[test]
fn instructions_stream_small_utf8_chunks_and_keep_the_exact_over_limit_count() {
    use bunshin_core::{
        Tuning,
        instructions::{InstructionsOrigin, InstructionsState},
    };
    let scratch = tempfile::tempdir().expect("scratch");
    let source = FileInstructions::new(scratch.path().into());
    let text = format!("{}終", "a文🦀b".repeat(30_000));
    source.ensure_default("").expect("private file");
    fs::write(source.path(), &text).expect("large UTF-8 file");
    let mut received = String::new();
    let present = source
        .read(&mut |chunk: &str| {
            assert!(chunk.len() <= 4096);
            received.push_str(chunk);
        })
        .expect("stream");
    assert!(present);
    assert_eq!(received, text);
    let state = InstructionsState::read(&source, Tuning::default()).expect("resolve");
    assert_eq!(state.origin, InstructionsOrigin::TooLong);
    assert_eq!(state.file_chars, Some(120_001));
    assert_eq!(
        state.text,
        bunshin_core::prompt::rules::DEFAULT_INSTRUCTIONS
    );
}

#[test]
fn a_partial_final_utf8_character_is_refused_after_valid_chunks() {
    use bunshin_core::{Tuning, instructions::InstructionsState};
    let scratch = tempfile::tempdir().expect("scratch");
    let source = FileInstructions::new(scratch.path().into());
    let mut bytes = "文".repeat(2000).into_bytes();
    bytes.extend_from_slice(&[0xe3, 0x81]);
    source.ensure_default("").expect("private file");
    fs::write(source.path(), &bytes).expect("incomplete UTF-8");
    assert_eq!(
        InstructionsState::read(&source, Tuning::default()),
        Err(InstructionsError::Unreadable)
    );
    assert_eq!(fs::read(source.path()).expect("preserved bytes"), bytes);
}

#[test]
fn insecure_instructions_permissions_are_refused_without_changing_them() {
    let scratch = tempfile::tempdir().expect("scratch");
    let root = scratch.path().join("data");
    let source = FileInstructions::new(root.clone());
    source.ensure_default("private text").expect("secure file");
    for (directory_mode, file_mode) in [(0o755, 0o600), (0o700, 0o644)] {
        fs::set_permissions(&root, fs::Permissions::from_mode(directory_mode)).expect("root mode");
        fs::set_permissions(source.path(), fs::Permissions::from_mode(file_mode))
            .expect("file mode");
        assert_eq!(
            bunshin_test_support::instructions_text(&source),
            Err(InstructionsError::Permissions)
        );
        assert_eq!(
            fs::metadata(&root)
                .expect("root unchanged")
                .permissions()
                .mode()
                & 0o777,
            directory_mode
        );
        assert_eq!(
            fs::metadata(source.path())
                .expect("file unchanged")
                .permissions()
                .mode()
                & 0o777,
            file_mode
        );
        assert_eq!(
            fs::read_to_string(source.path()).expect("bytes preserved"),
            "private text"
        );
    }
    source
        .ensure_default("not a replacement")
        .expect("explicit edit repairs permissions");
    assert_eq!(
        bunshin_test_support::instructions_text(&source),
        Ok(Some("private text".into()))
    );
}

#[test]
fn non_regular_instructions_have_an_actionable_unsafe_entry_error() {
    let scratch = tempfile::tempdir().expect("scratch");
    let source = FileInstructions::new(scratch.path().into());
    let external = scratch.path().join("external.md");
    fs::write(&external, "valid UTF-8").expect("external text");
    std::os::unix::fs::symlink(&external, source.path()).expect("file link");
    assert_eq!(
        bunshin_test_support::instructions_text(&source),
        Err(InstructionsError::UnsafeEntry)
    );
    assert_eq!(
        source.ensure_default("default"),
        Err(InstructionsError::UnsafeEntry)
    );
    assert_eq!(
        source.protect_after_edit(),
        Err(InstructionsError::UnsafeEntry)
    );
    assert_eq!(
        fs::read_to_string(&external).expect("preserved"),
        "valid UTF-8"
    );
}
