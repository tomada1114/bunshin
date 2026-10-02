//! Time-zone tests isolate `TZ` in child processes, without changing global state.

use std::{error::Error, process::Command};

use bunshin_core::Clock;
use bunshin_platform::SystemClock;
use jiff::{Timestamp, tz::TimeZone};

const PROBE_ENV: &str = "BUNSHIN_TEST_CLOCK_EXPECTATION";

#[test]
fn system_clock_honors_tz() -> Result<(), Box<dyn Error>> {
    for (zone, offset_millis) in [("UTC", "0"), ("Asia/Tokyo", "32400000")] {
        assert_in_child("system_clock_honors_tz", Some(zone), offset_millis)?;
    }
    Ok(())
}

#[test]
fn system_clock_uses_localtime_when_tz_is_unset() -> Result<(), Box<dyn Error>> {
    assert_in_child(
        "system_clock_uses_localtime_when_tz_is_unset",
        None,
        "localtime",
    )
}

fn assert_in_child(
    test_name: &str,
    zone: Option<&str>,
    expected: &str,
) -> Result<(), Box<dyn Error>> {
    if let Ok(probe) = std::env::var(PROBE_ENV) {
        let read = SystemClock.now();
        if probe == "localtime" {
            let tzif = std::fs::read("/etc/localtime")?;
            let zone = TimeZone::tzif("localtime", &tzif)?;
            let timestamp = Timestamp::from_millisecond(read.instant.0)?;
            assert_eq!(read.local, zone.to_datetime(timestamp));
        } else {
            let civil_as_utc = read
                .local
                .to_zoned(TimeZone::UTC)?
                .timestamp()
                .as_millisecond();
            assert_eq!(civil_as_utc - read.instant.0, probe.parse::<i64>()?);
        }
        return Ok(());
    }

    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--exact", test_name])
        .env(PROBE_ENV, expected)
        .env_remove("TZ");
    if let Some(zone) = zone {
        command.env("TZ", zone);
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "clock child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
