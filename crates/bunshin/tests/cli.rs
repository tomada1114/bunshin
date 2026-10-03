//! The built binary's command-line contract, with an isolated home directory.
use bunshin_core::instructions::InstructionsSource;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn command(home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bunshin"));
    command
        .args(args)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("VISUAL")
        .env_remove("EDITOR");
    command
}

fn output(mut command: Command) -> Output {
    match command.output() {
        Ok(output) => output,
        Err(error) => panic!("bunshin did not start: {error}"),
    }
}

fn run(home: &Path, args: &[&str]) -> Output {
    output(command(home, args))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn removed_sample_subcommand_is_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    for action in ["show", "increment", "--help"] {
        let result = run(home.path(), &[concat!("count", "er"), action]);
        assert_eq!(result.status.code(), Some(2), "{}", stderr(&result));
        assert_eq!(stdout(&result), "");
        assert!(stderr(&result).contains("Usage:"));
    }
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
}

#[test]
fn tui_without_a_terminal_fails_with_exit_code_1_and_touches_nothing() {
    let home = tempfile::tempdir().unwrap();
    let mut command = command(home.path(), &["tui"]);
    command.stdin(Stdio::null());
    let result = output(command);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "error: tui needs an interactive terminal on standard input and standard output\n"
    );
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
}

#[test]
fn legacy_sample_data_is_ignored_and_preserved() {
    let home = tempfile::tempdir().unwrap();
    #[cfg(target_os = "macos")]
    let directory = home
        .path()
        .join("Library/Application Support/io.github.tomada1114.bunshin");
    #[cfg(not(target_os = "macos"))]
    let directory = home.path().join(".local/share/bunshin");
    fs::create_dir_all(&directory).unwrap();
    let file = directory.join(concat!("count", "er.json"));
    let original = b"legacy bytes that this version must never parse";
    fs::write(&file, original).unwrap();
    for args in [&["--help"][..], &[concat!("count", "er"), "show"], &["tui"]] {
        run(home.path(), args);
        assert_eq!(fs::read(&file).unwrap(), original);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    }
}

#[test]
fn help_lists_the_tui_instructions_and_help_subcommands() {
    let home = tempfile::tempdir().unwrap();
    let result = run(home.path(), &["--help"]);
    assert_eq!(result.status.code(), Some(0));
    let help = stdout(&result);
    let commands: Vec<_> = help
        .split("Commands:\n")
        .nth(1)
        .unwrap()
        .split("\nOptions:")
        .next()
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    assert_eq!(commands, ["tui", "instructions", "help"]);
    assert_eq!(help.lines().next(), Some("ターミナルの秘書"));
    assert_eq!(stderr(&result), "");
}

#[test]
fn missing_instructions_print_the_default_and_reason_without_creating_files() {
    let home = tempfile::tempdir().expect("home");
    let result = run(home.path(), &["instructions"]);
    let path = bunshin_platform::app_data_dir(home.path()).join("instructions.md");
    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    assert_eq!(
        stdout(&result),
        bunshin_core::prompt::rules::DEFAULT_INSTRUCTIONS
    );
    assert_eq!(
        stderr(&result),
        format!(
            "既定の指示文を使っています（ファイルがありません）: {}\n",
            path.display()
        )
    );
    assert_eq!(fs::read_dir(home.path()).expect("no files").count(), 0);
}

#[test]
fn instructions_stdout_preserves_six_hundred_characters_without_a_final_newline() {
    let home = tempfile::tempdir().expect("home");
    let root = bunshin_platform::app_data_dir(home.path());
    let text = "文".repeat(600);
    bunshin_platform::FileInstructions::new(root.clone())
        .ensure_default(&text)
        .expect("private instructions");
    let path = root.join("instructions.md");
    let result = run(home.path(), &["instructions"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, text.as_bytes());
    assert_eq!(stdout(&result).chars().count(), 600);
    assert_eq!(
        stderr(&result),
        format!("ファイル: {}（600/600字）\n", path.display())
    );
    assert_eq!(fs::read(&path).expect("unchanged"), text.as_bytes());
}

#[test]
fn instructions_print_owner_text_and_file_character_count_on_separate_streams() {
    let home = tempfile::tempdir().expect("home");
    let directory = bunshin_platform::app_data_dir(home.path());
    fs::create_dir_all(&directory).expect("data");
    bunshin_platform::FileInstructions::new(directory.clone())
        .ensure_default("")
        .expect("private fixture");
    let path = directory.join("instructions.md");
    fs::write(&path, "日本語\n").expect("owner text");
    let result = run(home.path(), &["instructions"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(stdout(&result), "日本語\n");
    assert_eq!(
        stderr(&result),
        format!("ファイル: {}（4/600字）\n", path.display())
    );
    assert_eq!(fs::read_to_string(&path).expect("unchanged"), "日本語\n");
}

#[test]
fn over_limit_instructions_print_the_complete_default_and_reason() {
    let home = tempfile::tempdir().expect("home");
    let directory = bunshin_platform::app_data_dir(home.path());
    fs::create_dir_all(&directory).expect("data");
    bunshin_platform::FileInstructions::new(directory.clone())
        .ensure_default("")
        .expect("private fixture");
    let path = directory.join("instructions.md");
    fs::write(&path, "文".repeat(601)).expect("long text");
    let result = run(home.path(), &["instructions"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(
        stdout(&result),
        bunshin_core::prompt::rules::DEFAULT_INSTRUCTIONS
    );
    assert_eq!(
        stderr(&result),
        format!(
            "既定の指示文を使っています（ファイルの指示文が600字を超えています（601字））: {}\n",
            path.display()
        )
    );
    assert_eq!(
        fs::read_to_string(&path).expect("retained").chars().count(),
        601
    );
}

#[test]
fn instructions_edit_without_an_editor_fails_without_creating_a_file() {
    let home = tempfile::tempdir().expect("home");
    let path = if cfg!(target_os = "macos") {
        "~/Library/Application Support/io.github.tomada1114.bunshin/instructions.md"
    } else {
        "~/.local/share/bunshin/instructions.md"
    };
    let result = run(home.path(), &["instructions", "edit"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        format!(
            "error: エディタが設定されていません。VISUAL か EDITOR を設定するか、次のファイルを直接編集してください: {path}\n"
        )
    );
    assert_eq!(fs::read_dir(home.path()).expect("no files").count(), 0);
}

#[test]
fn instructions_edit_initializes_the_default_and_passes_flags_and_a_literal_path() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = tempfile::tempdir().expect("scratch");
    let home = scratch.path().join("home with 'quote' $(touch unexpected)");
    fs::create_dir(&home).expect("home");
    let editor = scratch.path().join("editor script.sh");
    fs::write(&editor, "test \"$1\" = --write || exit 9\ntest -s \"$2\" || exit 8\nprintf '%s' \"$2\" > \"$2.argument\"\nprintf '%s' '日本語' > \"$2\"\n").expect("scripted editor");
    let mut cmd = command(&home, &["instructions", "edit"]);
    cmd.env("VISUAL", format!("/bin/sh '{}' --write", editor.display()))
        .env("EDITOR", "/usr/bin/false");
    let result = output(cmd);
    let path = bunshin_platform::app_data_dir(&home).join("instructions.md");
    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "指示文を保存しました（3/600字）。次の呼び出しから使われます。\n"
    );
    assert_eq!(
        fs::read_to_string(path.with_extension("md.argument")).expect("literal argument"),
        path.to_string_lossy()
    );
    assert_eq!(fs::read_to_string(&path).expect("saved"), "日本語");
    assert_eq!(
        fs::metadata(&path).expect("mode").permissions().mode() & 0o777,
        0o600
    );
    assert!(!scratch.path().join("unexpected").exists());
    assert!(!home.join("unexpected").exists());
}

#[test]
fn empty_visual_uses_editor_and_reports_an_editor_failure_code() {
    let home = tempfile::tempdir().expect("home");
    let mut cmd = command(home.path(), &["instructions", "edit"]);
    cmd.env("VISUAL", "").env("EDITOR", "exit 7;");
    let result = output(cmd);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "error: エディタが終了コード7で終了しました。指示文ファイルを確認してください。\n"
    );
    let path = bunshin_platform::app_data_dir(home.path()).join("instructions.md");
    assert_eq!(
        fs::read_to_string(path).expect("default initialized"),
        bunshin_core::prompt::rules::DEFAULT_INSTRUCTIONS
    );
}

#[test]
fn instructions_edit_reports_and_retains_six_hundred_twelve_characters() {
    let home = tempfile::tempdir().expect("home");
    let editor = home.path().join("editor.sh");
    fs::write(
        &editor,
        "i=0; while [ \"$i\" -lt 612 ]; do printf x; i=$((i + 1)); done > \"$1\"\n",
    )
    .expect("scripted editor");
    let mut cmd = command(home.path(), &["instructions", "edit"]);
    cmd.env("VISUAL", format!("/bin/sh '{}'", editor.display()));
    let result = output(cmd);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "error: 指示文が600字を超えています（612字）。直すまでは既定の指示文が使われます。\n"
    );
    assert_eq!(
        fs::read_to_string(bunshin_platform::app_data_dir(home.path()).join("instructions.md"))
            .expect("retained")
            .chars()
            .count(),
        612
    );
}

#[test]
fn unreadable_instructions_fail_without_printing_text_or_changing_bytes() {
    let home = tempfile::tempdir().expect("home");
    let directory = bunshin_platform::app_data_dir(home.path());
    fs::create_dir_all(&directory).expect("data");
    bunshin_platform::FileInstructions::new(directory.clone())
        .ensure_default("")
        .expect("private fixture");
    let path = directory.join("instructions.md");
    fs::write(&path, [0xff, 0xfe]).expect("invalid UTF-8");
    let result = run(home.path(), &["instructions"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "error: 指示文をUTF-8として読めませんでした。ファイルはそのままです。\n"
    );
    assert_eq!(fs::read(&path).expect("unchanged"), [0xff, 0xfe]);
}

#[test]
fn version_prints_the_workspace_version_on_stdout() {
    let home = tempfile::tempdir().unwrap();
    let result = run(home.path(), &["--version"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(
        stdout(&result),
        format!("bunshin {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(stderr(&result), "");
}

#[test]
fn invalid_arguments_are_usage_errors_and_touch_nothing() {
    let home = tempfile::tempdir().unwrap();
    for args in [&["explode"][..], &[], &["tui", "--loud"]] {
        let result = run(home.path(), args);
        assert_eq!(result.status.code(), Some(2), "{}", stderr(&result));
        assert_eq!(stdout(&result), "");
        assert!(stderr(&result).contains("Usage:"));
    }
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
}

#[test]
fn editor_replacement_keeps_the_saved_instructions_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().expect("home");
    let script = home.path().join("editor.sh");
    fs::write(&script, "umask 022\nprintf '日本語' > \"$1.replacement\"\nchmod 644 \"$1.replacement\"\nmv \"$1.replacement\" \"$1\"\n").expect("script");
    let result = command(home.path(), &["instructions", "edit"])
        .env("VISUAL", format!("/bin/sh '{}'", script.display()))
        .output()
        .expect("edit");
    assert_eq!(result.status.code(), Some(0));
    let path = bunshin_platform::instructions_file(&bunshin_platform::app_data_dir(home.path()));
    assert_eq!(fs::read_to_string(&path).expect("saved"), "日本語");
    assert_eq!(
        fs::metadata(&path)
            .expect("permissions")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn empty_instructions_print_the_default_and_edited_empty_text_is_saved() {
    let home = tempfile::tempdir().expect("home");
    let path = bunshin_platform::instructions_file(&bunshin_platform::app_data_dir(home.path()));
    fs::create_dir_all(path.parent().expect("parent")).expect("data");
    bunshin_platform::FileInstructions::new(path.parent().expect("parent").into())
        .ensure_default("")
        .expect("private fixture");
    fs::write(&path, " \n").expect("whitespace");
    let result = run(home.path(), &["instructions"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(
        stdout(&result),
        bunshin_core::prompt::rules::DEFAULT_INSTRUCTIONS
    );
    assert_eq!(
        stderr(&result),
        format!(
            "既定の指示文を使っています（ファイルが空です（2/600字））: {}\n",
            path.display()
        )
    );
    let result = command(home.path(), &["instructions", "edit"])
        .env("VISUAL", "/usr/bin/true")
        .output()
        .expect("unchanged edit");
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "指示文を保存しました（2/600字）。次の呼び出しから使われます。\n"
    );
    assert_eq!(fs::read_to_string(&path).expect("preserved"), " \n");
}

#[test]
fn instructions_runtime_failures_print_no_text_and_exit_one() {
    let home = tempfile::tempdir().expect("home");
    let result = command(home.path(), &["instructions"])
        .env_remove("HOME")
        .output()
        .expect("missing home");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "error: HOME が設定されていないため、指示文ファイルの場所が分かりません。\n"
    );
    let path = bunshin_platform::instructions_file(&bunshin_platform::app_data_dir(home.path()));
    fs::create_dir_all(path.parent().expect("parent")).expect("data");
    fs::create_dir(&path).expect("invalid entry");
    let result = command(home.path(), &["instructions", "edit"])
        .env("VISUAL", "/usr/bin/true")
        .output()
        .expect("refuse");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "error: 指示文にはリンクや通常のファイル以外のものは使えません。通常のファイルに置き換えてください。ファイルはそのままです。\n"
    );
    fs::remove_dir(&path).expect("remove entry");
    let script = home.path().join("remove.sh");
    fs::write(&script, "rm \"$1\"\n").expect("script");
    let result = command(home.path(), &["instructions", "edit"])
        .env("VISUAL", format!("/bin/sh '{}'", script.display()))
        .output()
        .expect("remove edit");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "error: 編集後の指示文ファイルがありません。既定の指示文が使われます。\n"
    );
    assert!(!path.exists());
}

#[test]
fn closed_instructions_stdout_is_a_runtime_error_without_a_panic() {
    use std::os::{fd::OwnedFd, unix::net::UnixStream};
    let home = tempfile::tempdir().expect("home");
    let (reader, writer) = UnixStream::pair().expect("closed pipe");
    drop(reader);
    let result = command(home.path(), &["instructions"])
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .output()
        .expect("run");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stderr(&result), "error: 標準出力に書き込めませんでした。\n");
    assert_eq!(fs::read_dir(home.path()).expect("no files").count(), 0);
}

#[test]
fn failed_editor_replacement_also_keeps_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().expect("home");
    let script = home.path().join("fail.sh");
    fs::write(
        &script,
        "printf '日本語' > \"$1.new\"\nchmod 644 \"$1.new\"\nmv \"$1.new\" \"$1\"\nexit 7\n",
    )
    .expect("script");
    let result = command(home.path(), &["instructions", "edit"])
        .env("VISUAL", format!("/bin/sh '{}'", script.display()))
        .output()
        .expect("edit");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        stderr(&result),
        "error: エディタが終了コード7で終了しました。指示文ファイルを確認してください。\n"
    );
    let path = bunshin_platform::instructions_file(&bunshin_platform::app_data_dir(home.path()));
    assert_eq!(
        fs::metadata(path)
            .expect("permissions")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn instructions_refuse_insecure_modes_without_changing_owner_data_or_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().expect("home");
    let directory = bunshin_platform::app_data_dir(home.path());
    let source = bunshin_platform::FileInstructions::new(directory.clone());
    source
        .ensure_default("private owner instructions")
        .expect("secure fixture");
    for (directory_mode, file_mode) in [(0o755, 0o600), (0o700, 0o644)] {
        fs::set_permissions(&directory, fs::Permissions::from_mode(directory_mode))
            .expect("directory mode");
        fs::set_permissions(source.path(), fs::Permissions::from_mode(file_mode))
            .expect("file mode");
        let result = run(home.path(), &["instructions"]);
        assert_eq!(result.status.code(), Some(1));
        assert_eq!(stdout(&result), "");
        assert_eq!(
            stderr(&result),
            "error: 指示文の権限が必要な設定ではありません。保存先を0700、ファイルを0600にしてください。ファイルはそのままです。\n"
        );
        assert_eq!(
            fs::read_to_string(source.path()).expect("unchanged text"),
            "private owner instructions"
        );
        assert_eq!(
            fs::metadata(&directory)
                .expect("unchanged directory")
                .permissions()
                .mode()
                & 0o777,
            directory_mode
        );
        assert_eq!(
            fs::metadata(source.path())
                .expect("unchanged file")
                .permissions()
                .mode()
                & 0o777,
            file_mode
        );
    }
}

#[test]
fn the_no_editor_hint_does_not_disclose_the_private_home_path() {
    let scratch = tempfile::tempdir().expect("scratch");
    let home = scratch.path().join("private-owner-identity");
    fs::create_dir(&home).expect("home");
    let result = run(&home, &["instructions", "edit"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert!(!stderr(&result).contains("private-owner-identity"));
    assert!(stderr(&result).contains("~/"));
    assert!(stderr(&result).contains("instructions.md"));
    assert_eq!(fs::read_dir(home).expect("no files").count(), 0);
}

#[cfg(target_os = "linux")]
#[test]
fn the_no_editor_hint_refers_to_custom_xdg_storage_without_its_private_value() {
    let scratch = tempfile::tempdir().expect("scratch");
    let home = scratch.path().join("private-owner");
    fs::create_dir(&home).expect("home");
    let data = scratch.path().join("private-data");
    let result = command(&home, &["instructions", "edit"])
        .env("XDG_DATA_HOME", &data)
        .output()
        .expect("run");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert!(!stderr(&result).contains("private-owner"));
    assert!(!stderr(&result).contains("private-data"));
    assert!(stderr(&result).contains("$XDG_DATA_HOME/bunshin/instructions.md"));
    assert!(!data.exists());
    assert_eq!(fs::read_dir(home).expect("no files").count(), 0);
}

#[test]
fn editing_six_hundred_one_whitespace_characters_reports_over_limit_and_retains_the_file() {
    let home = tempfile::tempdir().expect("home");
    let source =
        bunshin_platform::FileInstructions::new(bunshin_platform::app_data_dir(home.path()));
    let text = " ".repeat(601);
    source.ensure_default(&text).expect("private fixture");
    let result = command(home.path(), &["instructions", "edit"])
        .env("VISUAL", "/usr/bin/true")
        .output()
        .expect("edit");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(stdout(&result), "");
    assert_eq!(
        stderr(&result),
        "error: 指示文が600字を超えています（601字）。直すまでは既定の指示文が使われます。\n"
    );
    assert_eq!(fs::read_to_string(source.path()).expect("preserved"), text);
}
