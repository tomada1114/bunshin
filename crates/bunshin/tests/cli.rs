//! The built binary's command-line contract, with an isolated home directory.
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
        "error: bunshin tui は端末の中で実行してください\n"
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
fn help_lists_only_the_tui_subcommand() {
    let home = tempfile::tempdir().unwrap();
    let result = run(home.path(), &["--help"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(stderr(&result), "");
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
    assert_eq!(commands, ["tui"]);
}

#[test]
fn removed_secretary_commands_are_usage_errors_and_leave_instructions_untouched() {
    let home = tempfile::tempdir().unwrap();
    let data = bunshin_platform::app_data_dir(home.path());
    fs::create_dir_all(&data).unwrap();
    let path = data.join("instructions.md");
    let original = b"owner's existing instructions";
    fs::write(&path, original).unwrap();

    for args in [
        &["today"][..],
        &["today", "--json"],
        &["instructions"],
        &["instructions", "edit"],
    ] {
        let result = run(home.path(), args);
        assert_eq!(result.status.code(), Some(2), "{}", stderr(&result));
        assert_eq!(stdout(&result), "");
        assert!(stderr(&result).contains("Usage:"));
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_dir(&data).unwrap().count(), 1);
    }
}
