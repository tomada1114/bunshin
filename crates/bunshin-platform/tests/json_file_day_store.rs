//! Atomic whole-day storage, private permissions, refused files, and process locks.
use bunshin_core::{
    Tuning,
    day::{
        Day,
        store::{DayStore, DayStoreError},
    },
};
use bunshin_platform::{JsonFileDayStore, days_dir, lock_file};
use jiff::civil::date;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
};

#[test]
fn missing_day_is_empty_and_readers_create_nothing() {
    let scratch = tempfile::tempdir().expect("scratch");
    let root = scratch.path().join("absent");
    let store = JsonFileDayStore::new(root.clone(), Tuning::default());
    assert_eq!(
        store.load(date(2026, 10, 2)).expect("empty").data(),
        Day::new(date(2026, 10, 2), Tuning::default()).data()
    );
    assert_eq!(store.last_before(date(2026, 10, 2)), Ok(None));
    assert!(!root.exists());
}

#[test]
fn dangling_day_symlinks_are_refused_and_preserved() {
    let scratch = tempfile::tempdir().expect("scratch");
    let store = JsonFileDayStore::new(scratch.path().into(), Tuning::default());
    fs::create_dir_all(days_dir(scratch.path())).expect("days");
    let path = days_dir(scratch.path()).join("2026-10-02.json");
    let missing = scratch.path().join("missing-target.json");
    std::os::unix::fs::symlink(&missing, &path).expect("dangling day entry");
    assert_eq!(
        store.load(date(2026, 10, 2)),
        Err(DayStoreError::Unreadable)
    );
    assert_eq!(
        store.save(&Day::new(date(2026, 10, 2), Tuning::default())),
        Err(DayStoreError::Unreadable)
    );
    assert_eq!(
        store.last_before(date(2026, 10, 3)),
        Err(DayStoreError::Unreadable)
    );
    assert!(
        fs::symlink_metadata(&path)
            .expect("entry preserved")
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(&path).expect("target preserved"), missing);
    assert!(!missing.exists());
}

#[test]
fn save_replaces_the_whole_day_and_creates_private_directories_and_files() {
    let scratch = tempfile::tempdir().expect("scratch");
    let root = bunshin_platform::macos_data_dir(scratch.path());
    let store = JsonFileDayStore::new(root.clone(), Tuning::default());
    let day = Day::new(date(2026, 10, 2), Tuning::default());
    store.save(&day).expect("save");
    let path = days_dir(&root).join("2026-10-02.json");
    assert_eq!(
        fs::metadata(&root).expect("root").permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(days_dir(&root))
            .expect("days")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&path).expect("file").permissions().mode() & 0o777,
        0o600
    );
    let bytes = fs::read(&path).expect("bytes");
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(json["format"], 1);
    assert_eq!(json["date"], "2026-10-02");
    assert_eq!(fs::read_dir(days_dir(&root)).expect("files").count(), 1);
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).expect("widen root");
    fs::set_permissions(days_dir(&root), fs::Permissions::from_mode(0o755)).expect("widen days");
    let guard = store.take_lock().expect("lock at startup");
    assert_eq!(
        fs::metadata(&root).expect("root").permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(days_dir(&root))
            .expect("days")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(lock_file(&root))
            .expect("lock file")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    drop(guard);
}

#[test]
fn corrupt_and_future_files_are_refused_and_never_overwritten() {
    let scratch = tempfile::tempdir().expect("scratch");
    let store = JsonFileDayStore::new(scratch.path().into(), Tuning::default());
    fs::create_dir_all(days_dir(scratch.path())).expect("days");
    let path = days_dir(scratch.path()).join("2026-10-02.json");
    let day = Day::new(date(2026, 10, 2), Tuning::default());
    for (bytes, error) in [
        (b"not json".as_slice(), DayStoreError::Unreadable),
        (
            br#"{"format":2,"future":"different shape"}"#.as_slice(),
            DayStoreError::NewerFormat { found: 2 },
        ),
        (
            br#"{"format":0}"#.as_slice(),
            DayStoreError::UnsupportedFormat { found: 0 },
        ),
        (br#"{"format":1}"#.as_slice(), DayStoreError::Unreadable),
    ] {
        fs::write(&path, bytes).expect("fixture");
        bunshin_test_support::day_store_refusal_contract(&store, day.date(), error);
        assert_eq!(store.load(day.date()), Err(error));
        assert_eq!(store.save(&day), Err(error));
        assert_eq!(fs::read(&path).expect("unchanged"), bytes);
    }
    let wrong = Day::new(date(2026, 10, 1), Tuning::default());
    let wrong_bytes =
        serde_json::to_vec(&bunshin_core::day::file::DayFile::from(&wrong)).expect("fixture");
    fs::write(&path, &wrong_bytes).expect("fixture");
    assert_eq!(store.load(day.date()), Err(DayStoreError::Unreadable));
    assert_eq!(store.save(&day), Err(DayStoreError::Unreadable));
    assert_eq!(fs::read(&path).expect("unchanged"), wrong_bytes);
}

#[test]
fn readers_only_see_complete_old_or_new_days_during_replacement() {
    use bunshin_core::{
        UnixMillis,
        day::{TaskKind, TaskOrigin},
    };
    let scratch = tempfile::tempdir().expect("scratch");
    let store = JsonFileDayStore::new(scratch.path().into(), Tuning::default());
    let old = Day::new(date(2026, 10, 2), Tuning::default());
    let mut new = old.clone();
    for title in ["one", "two", "three"] {
        new = new
            .add(
                title.into(),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                UnixMillis(42),
            )
            .expect("add")
            .0;
    }
    store.save(&old).expect("initial");
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for _ in 0..100 {
                let day = store.load(old.date()).expect("complete file");
                assert!(day.data() == old.data() || day.data() == new.data());
            }
        });
        for _ in 0..20 {
            store.save(&new).expect("new");
            store.save(&old).expect("old");
        }
    });
    assert_eq!(
        fs::read_dir(days_dir(scratch.path()))
            .expect("no leftovers")
            .count(),
        1
    );
}

#[test]
fn latest_before_ignores_temporary_and_unrecognized_names_but_refuses_a_bad_selected_day() {
    let scratch = tempfile::tempdir().expect("scratch");
    let store = JsonFileDayStore::new(scratch.path().into(), Tuning::default());
    store
        .save(&Day::new(date(2026, 9, 28), Tuning::default()))
        .expect("older");
    for name in ["2026-10-01.json.1.tmp", "notes.json", "2026-1-1.json"] {
        fs::write(days_dir(scratch.path()).join(name), "incomplete").expect("ignored fixture");
    }
    assert_eq!(
        store
            .last_before(date(2026, 10, 2))
            .expect("latest")
            .expect("day")
            .date(),
        date(2026, 9, 28)
    );
    fs::write(days_dir(scratch.path()).join("2026-10-01.json"), "invalid")
        .expect("unreadable latest");
    assert_eq!(
        store.last_before(date(2026, 10, 2)),
        Err(DayStoreError::Unreadable)
    );
}

#[test]
fn failed_save_keeps_the_old_file() {
    let scratch = tempfile::tempdir().expect("scratch");
    let root = scratch.path().join("data");
    let store = JsonFileDayStore::new(root.clone(), Tuning::default());
    let day = Day::new(date(2026, 10, 2), Tuning::default());
    store.save(&day).expect("first save");
    let path = days_dir(&root).join("2026-10-02.json");
    let old = fs::read(&path).expect("old");
    // A read-only mount cannot be narrowed to 0700; a file in place of the root
    // likewise refuses the save without touching the already persisted day.
    let saved_root = scratch.path().join("preserved");
    fs::rename(&root, &saved_root).expect("preserve tree");
    fs::write(&root, "not a directory").expect("block root");
    assert_eq!(store.save(&day), Err(DayStoreError::Unavailable));
    assert_eq!(
        fs::read(days_dir(&saved_root).join("2026-10-02.json")).expect("old intact"),
        old
    );
}

#[test]
fn independent_handles_report_the_holder_and_release_on_drop() {
    let scratch = tempfile::tempdir().expect("scratch");
    let first = JsonFileDayStore::new(scratch.path().into(), Tuning::default());
    let second = JsonFileDayStore::new(scratch.path().into(), Tuning::default());
    let guard = first.take_lock().expect("first lock");
    assert!(
        matches!(second.take_lock(), Err(DayStoreError::AlreadyLocked { pid: Some(pid) }) if pid == std::process::id())
    );
    fs::write(lock_file(scratch.path()), "invalid pid").expect("unreadable pid");
    assert!(matches!(
        second.take_lock(),
        Err(DayStoreError::AlreadyLocked { pid: None })
    ));
    drop(guard);
    let guard = second.take_lock().expect("released");
    assert_eq!(
        fs::read_to_string(lock_file(scratch.path()))
            .expect("pid")
            .trim(),
        std::process::id().to_string()
    );
    drop(guard);
}

#[test]
fn a_symlinked_lock_never_changes_the_external_target() {
    let scratch = tempfile::tempdir().expect("scratch");
    let root = scratch.path().join("data");
    fs::create_dir(&root).expect("root");
    let outside = scratch.path().join("owner-document.txt");
    let bytes = b"owner document must remain unchanged";
    fs::write(&outside, bytes).expect("external document");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o640)).expect("original mode");
    std::os::unix::fs::symlink(&outside, lock_file(&root)).expect("symlink lock");
    let store = JsonFileDayStore::new(root.clone(), Tuning::default());
    assert!(matches!(store.take_lock(), Err(DayStoreError::Unavailable)));
    assert_eq!(fs::read(&outside).expect("unchanged bytes"), bytes);
    assert_eq!(
        fs::metadata(&outside)
            .expect("unchanged mode")
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    assert_eq!(
        fs::read_link(lock_file(&root)).expect("unchanged link"),
        outside
    );
}

#[test]
fn a_dangling_lock_symlink_never_creates_its_target() {
    let scratch = tempfile::tempdir().expect("scratch");
    let outside = scratch.path().join("missing-document.txt");
    std::os::unix::fs::symlink(&outside, lock_file(scratch.path())).expect("dangling lock");
    let store = JsonFileDayStore::new(scratch.path().into(), Tuning::default());
    assert!(matches!(store.take_lock(), Err(DayStoreError::Unavailable)));
    assert!(!outside.exists());
    assert_eq!(
        fs::read_link(lock_file(scratch.path())).expect("unchanged link"),
        outside
    );
}

#[test]
fn a_lock_with_an_external_hard_link_is_refused() {
    let scratch = tempfile::tempdir().expect("scratch");
    let root = scratch.path().join("data");
    fs::create_dir(&root).expect("root");
    let outside = scratch.path().join("owner-document.txt");
    fs::write(&outside, b"unchanged").expect("external document");
    fs::hard_link(&outside, lock_file(&root)).expect("hard link");
    let store = JsonFileDayStore::new(root, Tuning::default());
    assert!(matches!(store.take_lock(), Err(DayStoreError::Unavailable)));
    assert_eq!(fs::read(&outside).expect("unchanged"), b"unchanged");
}

#[test]
fn child_lock_holder() {
    let Some(root) = std::env::var_os("BUNSHIN_TEST_LOCK_ROOT") else {
        return;
    };
    let store = JsonFileDayStore::new(root.into(), Tuning::default());
    let _guard = store.take_lock().expect("child lock");
    println!("LOCKED {}", std::process::id());
    std::io::stdout().flush().expect("handshake");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).expect("parent pipe");
}

#[test]
fn lock_is_released_when_a_process_is_killed() {
    let scratch = tempfile::tempdir().expect("scratch");
    let mut child = Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "child_lock_holder", "--nocapture"])
        .env("BUNSHIN_TEST_LOCK_ROOT", scratch.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("child");
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));
    let mut line = String::new();
    loop {
        assert_ne!(
            output.read_line(&mut line).expect("handshake"),
            0,
            "child exited before lock"
        );
        if line.starts_with("LOCKED ") {
            break;
        }
        line.clear();
    }
    let store = JsonFileDayStore::new(scratch.path().into(), Tuning::default());
    let refused = store.take_lock();
    child.kill().expect("simulate crash");
    child.wait().expect("reap");
    assert!(
        matches!(refused, Err(DayStoreError::AlreadyLocked { pid: Some(pid) }) if pid == child.id())
    );
    let _guard = store.take_lock().expect("OS released lock");
}
