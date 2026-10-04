use bunshin_core::{
    Availability, CancelFlag, LanguageModel, ModelAnswer, ModelError, ModelRequest,
    UnavailableReason,
};

/// The Linux model adapter: all model operations remain unavailable on this OS.
#[derive(Debug, Default)]
pub struct UnavailableLanguageModel;
impl LanguageModel for UnavailableLanguageModel {
    fn availability(&self) -> Result<Availability, ModelError> {
        Ok(Availability::Unavailable(UnavailableReason::UnsupportedOs))
    }
    fn respond(&self, _: &ModelRequest, cancel: &CancelFlag) -> Result<ModelAnswer, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        Err(ModelError::Unavailable(UnavailableReason::UnsupportedOs))
    }
}

#[cfg(any(target_os = "macos", test))]
mod command {
    use super::{
        Availability, CancelFlag, LanguageModel, ModelAnswer, ModelError, ModelRequest,
        UnavailableReason,
    };
    use std::{
        io::{self, Read, Write},
        os::unix::process::CommandExt,
        path::PathBuf,
        process::{Child, Command, ExitStatus, Stdio},
        thread,
        time::{Duration, Instant},
    };

    const FM_PATH: &str = "/usr/bin/fm";
    const KILL_PATH: &str = "/bin/kill";
    const POLL_INTERVAL: Duration = Duration::from_millis(5);

    /// A fresh `fm` process per call, never a shell. Routine tests inject a stub path.
    #[derive(Debug)]
    pub struct FmLanguageModel {
        executable: PathBuf,
        probe_timeout: Duration,
    }
    impl Default for FmLanguageModel {
        fn default() -> Self {
            Self::new(FM_PATH.into())
        }
    }
    impl FmLanguageModel {
        /// Use an absolute command path, allowing harmless stub commands in tests.
        #[must_use]
        pub fn new(executable: PathBuf) -> Self {
            Self::with_tuning(executable, bunshin_core::Tuning::default())
        }
        /// Use a literal command path and the root timeout for availability probes.
        /// Each response still uses its request's timeout, built from the same tuning.
        #[must_use]
        pub fn with_tuning(executable: PathBuf, tuning: bunshin_core::Tuning) -> Self {
            Self {
                executable,
                probe_timeout: tuning.model_timeout,
            }
        }
        #[cfg(test)]
        pub(super) fn executable_for_test(&self) -> PathBuf {
            self.executable.clone()
        }

        fn run(
            &self,
            args: &[&str],
            prompt: &str,
            timeout: Duration,
            cancel: &CancelFlag,
        ) -> Result<(ExitStatus, Vec<u8>), ModelError> {
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            let started = Instant::now();
            let mut child = Command::new(&self.executable)
                .args(args)
                .process_group(0)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| spawn_error(&error))?;
            // All streams are configured as pipes above. If taking one ever fails,
            // terminate and reap before returning rather than abandon the process.
            let (Some(mut stdin), Some(mut stdout), Some(mut stderr)) =
                (child.stdin.take(), child.stdout.take(), child.stderr.take())
            else {
                terminate(&mut child)?;
                return Err(ModelError::Failed);
            };
            thread::scope(|scope| {
                let Ok(writer) = thread::Builder::new()
                    .spawn_scoped(scope, move || stdin.write_all(prompt.as_bytes()))
                else {
                    terminate(&mut child)?;
                    return Err(ModelError::Failed);
                };
                let Ok(reader) = thread::Builder::new().spawn_scoped(scope, move || {
                    let mut bytes = Vec::new();
                    stdout.read_to_end(&mut bytes).map(|_| bytes)
                }) else {
                    terminate(&mut child)?;
                    return Err(ModelError::Failed);
                };
                // Stderr can contain user text. Drain it concurrently, retaining none.
                let Ok(diagnostics) = thread::Builder::new()
                    .spawn_scoped(scope, move || io::copy(&mut stderr, &mut io::sink()))
                else {
                    terminate(&mut child)?;
                    return Err(ModelError::Failed);
                };
                let status = wait(&mut child, started, timeout, cancel, || {
                    writer.is_finished() && reader.is_finished() && diagnostics.is_finished()
                });
                let written = writer.join().map_err(|_| ModelError::Failed)?;
                let output = reader.join().map_err(|_| ModelError::Failed)?;
                let drained = diagnostics.join().map_err(|_| ModelError::Failed)?;
                let status = status?;
                if !status.success() {
                    tracing::warn!(code = status.code(), "model command failed");
                    return Ok((status, Vec::new()));
                }
                written.map_err(|_| ModelError::Failed)?;
                drained.map_err(|_| ModelError::Failed)?;
                Ok((status, output.map_err(|_| ModelError::Failed)?))
            })
        }
    }
    fn spawn_error(error: &io::Error) -> ModelError {
        if error.kind() == io::ErrorKind::NotFound {
            ModelError::Unavailable(UnavailableReason::NotInstalled)
        } else {
            ModelError::Failed
        }
    }
    fn terminate(child: &mut Child) -> Result<(), ModelError> {
        terminate_using(child, KILL_PATH)
    }
    pub(super) fn terminate_using(child: &mut Child, kill_path: &str) -> Result<(), ModelError> {
        // The root is not reaped until all streams finish or cleanup completes, so
        // its PID still identifies the private group this call created.
        let group = format!("-{}", child.id());
        let group_status = Command::new(kill_path)
            .args(["-KILL", "--", &group])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        // Even if the group helper fails, try to stop and reap the directly owned
        // child. This does not promise bounded drainage for escaped descendants or
        // when the OS refuses the group signal.
        let direct_killed = child.kill();
        let waited = child.wait().map_err(|_| ModelError::Failed)?;
        if direct_killed.is_err() && waited.success() {
            tracing::debug!("model exited during termination");
        }
        match group_status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => {
                tracing::warn!(code = status.code(), "model group termination failed");
                Err(ModelError::Failed)
            }
            Err(_) => {
                tracing::warn!("model group termination command failed");
                Err(ModelError::Failed)
            }
        }
    }
    fn wait(
        child: &mut Child,
        started: Instant,
        timeout: Duration,
        cancel: &CancelFlag,
        streams_finished: impl Fn() -> bool,
    ) -> Result<ExitStatus, ModelError> {
        loop {
            // Completed streams plus an exited child win over a late polling wakeup.
            // Do not reap an exited root while descendants still own pipe ends:
            // cleanup must retain ownership of the root PID and private group.
            if streams_finished() {
                match child.try_wait() {
                    Ok(Some(status)) => return Ok(status),
                    Ok(None) => {}
                    Err(_) => {
                        terminate(child)?;
                        return Err(ModelError::Failed);
                    }
                }
            }
            let reason = if cancel.is_cancelled() {
                Some(ModelError::Cancelled)
            } else if started.elapsed() >= timeout {
                Some(ModelError::TimedOut)
            } else {
                None
            };
            if let Some(reason) = reason {
                terminate(child)?;
                return Err(reason);
            }
            thread::sleep(POLL_INTERVAL.min(timeout.saturating_sub(started.elapsed())));
        }
    }
    impl LanguageModel for FmLanguageModel {
        fn availability(&self) -> Result<Availability, ModelError> {
            self.availability_with_cancel(&CancelFlag::default())
        }
        fn availability_with_cancel(
            &self,
            cancel: &CancelFlag,
        ) -> Result<Availability, ModelError> {
            let result = self.run(&["available"], "", self.probe_timeout, cancel);
            match result {
                Err(ModelError::Unavailable(reason)) => Ok(Availability::Unavailable(reason)),
                Err(error) => Err(error),
                Ok((status, _)) if status.success() => Ok(Availability::Available),
                Ok((status, _)) if status.code() == Some(69) => Ok(Availability::Unavailable(
                    UnavailableReason::TermsNotAccepted,
                )),
                Ok(_) => Err(ModelError::Failed),
            }
        }
        fn respond(
            &self,
            request: &ModelRequest,
            cancel: &CancelFlag,
        ) -> Result<ModelAnswer, ModelError> {
            let (status, output) = self.run(
                &[
                    "respond",
                    "--no-stream",
                    "--schema",
                    &request.schema,
                    "--instructions",
                    &request.instructions,
                ],
                &request.prompt,
                request.timeout,
                cancel,
            )?;
            if status.code() == Some(69) {
                return Err(ModelError::Unavailable(UnavailableReason::TermsNotAccepted));
            }
            if !status.success() {
                return Err(ModelError::Failed);
            }
            let json = String::from_utf8(output).map_err(|_| ModelError::Malformed)?;
            serde_json::from_str::<serde_json::Value>(&json).map_err(|_| ModelError::Malformed)?;
            Ok(ModelAnswer { json })
        }
    }
}
#[cfg(target_os = "macos")]
pub use command::FmLanguageModel;

#[cfg(test)]
mod tests {
    use super::{command::FmLanguageModel, *};
    use bunshin_test_support::language_model_contract;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::Command,
        thread,
        time::{Duration, Instant},
    };

    fn stub(body: &str) -> (tempfile::TempDir, FmLanguageModel) {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let path = dir.path().join("fm-stub");
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))
            .unwrap_or_else(|error| panic!("stub: {error}"));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|error| panic!("permissions: {error}"));
        let model = FmLanguageModel::new(path);
        (dir, model)
    }
    fn request() -> ModelRequest {
        ModelRequest {
            instructions: "rules ' \"\nnext".into(),
            prompt: "task ' \"\n続き\n".into(),
            schema: r#"{"type":"object"}"#.into(),
            timeout: Duration::from_secs(5),
        }
    }
    #[test]
    fn stub_meets_model_contract() {
        let (_dir, model) =
            stub("if [ \"$1\" = available ]; then exit 0; fi\n/bin/cat >/dev/null\nprintf '{}' ");
        language_model_contract(|| Box::new(FmLanguageModel::new(model.executable_for_test())));
    }
    #[test]
    fn prompt_is_unchanged_on_stdin_and_absent_from_argv() {
        let (dir, model) = stub(
            r#"cd "$(dirname "$0")"
printf '%s\000' "$@" >args
/bin/cat >input
printf '{"reply":"了解"}'"#,
        );
        let req = request();
        assert_eq!(
            model.respond(&req, &CancelFlag::default()).unwrap().json,
            r#"{"reply":"了解"}"#
        );
        assert_eq!(
            fs::read(dir.path().join("input")).unwrap(),
            req.prompt.as_bytes()
        );
        let args = fs::read(dir.path().join("args")).unwrap();
        let expected = [
            "respond",
            "--no-stream",
            "--schema",
            &req.schema,
            "--instructions",
            &req.instructions,
        ]
        .join("\0")
            + "\0";
        assert_eq!(args, expected.as_bytes());
    }
    #[test]
    fn missing_command_and_probe_exit_codes_are_typed() {
        let dir = tempfile::tempdir().unwrap();
        let absent = FmLanguageModel::new(dir.path().join("absent"));
        assert_eq!(
            absent.availability(),
            Ok(Availability::Unavailable(UnavailableReason::NotInstalled))
        );
        assert_eq!(
            absent.respond(&request(), &CancelFlag::default()),
            Err(ModelError::Unavailable(UnavailableReason::NotInstalled))
        );
        for (code, expected) in [
            (0, Ok(Availability::Available)),
            (
                69,
                Ok(Availability::Unavailable(
                    UnavailableReason::TermsNotAccepted,
                )),
            ),
            (1, Err(ModelError::Failed)),
            (64, Err(ModelError::Failed)),
            (73, Err(ModelError::Failed)),
        ] {
            let (_dir, model) = stub(&format!("exit {code}"));
            assert_eq!(model.availability(), expected);
        }
    }
    #[test]
    fn nonzero_exit_and_invalid_json_do_not_expose_diagnostics() {
        for code in [1, 64, 73] {
            let (_dir, model) = stub(&format!(
                "/bin/cat >/dev/null\nprintf 'private task' >&2\nexit {code}"
            ));
            assert_eq!(
                model.respond(&request(), &CancelFlag::default()),
                Err(ModelError::Failed)
            );
        }
        let (_dir, model) = stub("/bin/cat >/dev/null\nexit 69");
        assert_eq!(
            model.respond(&request(), &CancelFlag::default()),
            Err(ModelError::Unavailable(UnavailableReason::TermsNotAccepted))
        );
        for text in ["", "invalid", "{} trailing"] {
            let (_dir, model) = stub(&format!("/bin/cat >/dev/null\nprintf '%s' '{text}'"));
            assert_eq!(
                model.respond(&request(), &CancelFlag::default()),
                Err(ModelError::Malformed)
            );
        }
        let (_dir, model) = stub("/bin/cat >/dev/null\nprintf '\\377'");
        assert_eq!(
            model.respond(&request(), &CancelFlag::default()),
            Err(ModelError::Malformed)
        );
    }
    #[test]
    fn pre_cancelled_call_never_spawns() {
        let (dir, model) = stub("touch \"$(dirname \"$0\")/spawned\"");
        let cancel = CancelFlag::default();
        cancel.clone().cancel();
        assert_eq!(
            model.respond(&request(), &cancel),
            Err(ModelError::Cancelled)
        );
        assert!(!dir.path().join("spawned").exists());
    }
    #[test]
    fn pre_cancelled_probe_never_spawns() {
        let (dir, model) = stub("touch \"$(dirname \"$0\")/spawned\"");
        let cancel = CancelFlag::default();
        cancel.cancel();
        assert_eq!(
            model.availability_with_cancel(&cancel),
            Err(ModelError::Cancelled)
        );
        assert!(!dir.path().join("spawned").exists());
    }
    #[test]
    fn cancellation_kills_and_reaps_running_probe() {
        let (dir, model) =
            stub("printf '%s\\n' \"$$\" >\"$(dirname \"$0\")/pid\"\nwhile :; do :; done");
        let cancel = CancelFlag::default();
        thread::scope(|scope| {
            let call = scope.spawn(|| model.availability_with_cancel(&cancel));
            let start = Instant::now();
            while !fs::read_to_string(dir.path().join("pid")).is_ok_and(|pid| pid.ends_with('\n')) {
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "probe never started"
                );
                thread::yield_now();
            }
            cancel.cancel();
            assert_eq!(call.join().unwrap(), Err(ModelError::Cancelled));
        });
        assert_reaped(&dir);
    }
    fn assert_reaped(dir: &tempfile::TempDir) {
        let pid = fs::read_to_string(dir.path().join("pid"))
            .unwrap_or_else(|error| panic!("pid: {error}"));
        let status = Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap_or_else(|error| panic!("kill probe: {error}"));
        assert!(!status.success(), "child remains alive or unreaped");
    }
    #[test]
    fn timeout_kills_and_reaps_even_with_blocked_stdin() {
        let (dir, model) =
            stub("printf '%s' \"$$\" >\"$(dirname \"$0\")/pid\"\nexec /bin/sleep 30");
        let mut req = request();
        req.prompt = "x".repeat(2_000_000);
        req.timeout = Duration::from_millis(200);
        assert_eq!(
            model.respond(&req, &CancelFlag::default()),
            Err(ModelError::TimedOut)
        );
        assert_reaped(&dir);
    }
    #[test]
    fn timeout_still_applies_after_child_exit_while_inherited_pipes_remain_open() {
        let (_dir, model) = stub("/bin/cat >/dev/null\n/bin/sleep 1 &\nprintf '{}'");
        let mut req = request();
        req.timeout = Duration::from_millis(200);
        assert_eq!(
            model.respond(&req, &CancelFlag::default()),
            Err(ModelError::TimedOut)
        );
    }
    #[test]
    fn cancellation_kills_and_reaps_running_child() {
        let (dir, model) =
            stub("printf '%s' \"$$\" >\"$(dirname \"$0\")/pid\"\nexec /bin/sleep 30");
        let cancel = CancelFlag::default();
        thread::scope(|scope| {
            let call = scope.spawn(|| model.respond(&request(), &cancel));
            let start = Instant::now();
            while !dir.path().join("pid").exists() {
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "child never started"
                );
                thread::yield_now();
            }
            cancel.cancel();
            assert_eq!(call.join().unwrap(), Err(ModelError::Cancelled));
        });
        assert_reaped(&dir);
    }
    #[test]
    fn concurrent_large_input_stdout_and_stderr_do_not_deadlock() {
        let (_dir, model) = stub(
            "/usr/bin/awk 'BEGIN {for(i=0;i<200000;i++) printf \"x\"}' >&2\nprintf '\"'\n/usr/bin/awk 'BEGIN {for(i=0;i<200000;i++) printf \"x\"}'\nprintf '\"'\n/bin/cat >/dev/null",
        );
        let mut req = request();
        req.prompt = "y".repeat(2_000_000);
        let answer = model.respond(&req, &CancelFlag::default()).unwrap();
        assert_eq!(answer.json.len(), 200_002);
    }
    #[test]
    fn non_executable_command_and_signal_exit_are_failed() {
        let (dir, model) = stub("exit 0");
        fs::set_permissions(
            dir.path().join("fm-stub"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert_eq!(model.availability(), Err(ModelError::Failed));
        assert_eq!(
            model.respond(&request(), &CancelFlag::default()),
            Err(ModelError::Failed)
        );
        let (_dir, model) = stub("kill -TERM $$");
        assert_eq!(
            model.respond(&request(), &CancelFlag::default()),
            Err(ModelError::Failed)
        );
    }
    #[test]
    fn any_valid_json_is_preserved_for_core_validation() {
        for json in ["null", "[]", "42", "true", "  {}\n"] {
            let (_dir, model) = stub(&format!("/bin/cat >/dev/null\nprintf '%s' '{json}'"));
            assert_eq!(
                model.respond(&request(), &CancelFlag::default()),
                Ok(ModelAnswer { json: json.into() })
            );
        }
    }
    #[test]
    fn failure_logs_only_the_exit_code_and_never_model_text() {
        use std::sync::{Arc, Mutex, PoisonError};
        #[derive(Clone)]
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let capture = Capture(Arc::clone(&bytes));
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || capture.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        let (_dir, model) = stub("/bin/cat >&2\nprintf 'private diagnostics' >&2\nexit 73");
        let mut req = request();
        req.instructions = "private rules".into();
        req.prompt = "private task".into();
        assert_eq!(
            model.respond(&req, &CancelFlag::default()),
            Err(ModelError::Failed)
        );
        let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert!(log.contains("code=73"));
        assert!(!log.contains("private"));
    }
    #[test]
    fn availability_uses_custom_tuning_timeout_and_reaps_child() {
        let (dir, model) =
            stub("printf '%s' \"$$\" >\"$(dirname \"$0\")/pid\"\nexec /bin/sleep 30");
        let tuning = bunshin_core::Tuning {
            model_timeout: Duration::from_millis(200),
            ..bunshin_core::Tuning::default()
        };
        let model = FmLanguageModel::with_tuning(model.executable_for_test(), tuning);
        assert_eq!(model.availability(), Err(ModelError::TimedOut));
        assert_reaped(&dir);
    }
    #[test]
    fn timeout_applies_when_only_descendant_stderr_remains_open() {
        let (dir, model) = stub(
            r#"cd "$(dirname "$0")"
printf '%s' "$$" >pid
/bin/cat >/dev/null
/bin/sh -c '/bin/sleep 1; printf completed >natural-end' >/dev/null &
printf '{}'"#,
        );
        let mut req = request();
        req.timeout = Duration::from_millis(200);
        assert_eq!(
            model.respond(&req, &CancelFlag::default()),
            Err(ModelError::TimedOut)
        );
        assert!(
            !dir.path().join("natural-end").exists(),
            "pipe owner reached its natural lifetime before call returned"
        );
        assert_reaped(&dir);
    }
    #[test]
    fn timeout_applies_when_only_descendant_stdin_blocks_the_writer() {
        let (dir, model) = stub(
            r#"cd "$(dirname "$0")"
printf '%s' "$$" >pid
# Preserve stdin before an asynchronous shell list replaces fd 0 with /dev/null.
exec 3<&0
/bin/sh -c '/bin/sleep 1; printf completed >natural-end' <&3 >/dev/null 2>&1 &
printf '{}'"#,
        );
        let mut req = request();
        req.prompt = "x".repeat(2_000_000);
        req.timeout = Duration::from_millis(200);
        assert_eq!(
            model.respond(&req, &CancelFlag::default()),
            Err(ModelError::TimedOut)
        );
        assert!(
            !dir.path().join("natural-end").exists(),
            "stdin owner reached its natural lifetime before call returned"
        );
        assert_reaped(&dir);
    }

    #[test]
    fn group_helper_spawn_or_status_failure_still_kills_and_reaps_root() {
        use std::os::unix::process::{CommandExt, ExitStatusExt};
        let (dir, helper) = stub("exit 73");
        let absent = dir.path().join("missing-helper");
        for path in [absent, helper.executable_for_test()] {
            let mut child = Command::new("/bin/sleep")
                .arg("1")
                .process_group(0)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            assert_eq!(
                super::command::terminate_using(&mut child, path.to_str().unwrap()),
                Err(ModelError::Failed)
            );
            // wait() returns the cached reaped status, proving fallback sent SIGKILL.
            assert_eq!(child.wait().unwrap().signal(), Some(9));
        }
    }
}
