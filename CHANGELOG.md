# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A whole-day JSON store with private files, atomic replacement, and an OS-managed
  writer lease. `PublishedButNotDurable` distinguishes a visible replacement whose
  directory sync failed; day files remain unlimited and lock PIDs are advisory.

- A synchronous on-device model port with a macOS `fm` adapter, timeout and
  cancellation, typed availability, and a scripted fake for app development.

- Local civil time alongside each clock instant, with a tunable 04:00 boundary for
  the logical day and fixed-offset clocks for deterministic tests.

### Removed

- **Breaking:** removed the counter sample, its public APIs, and the `counter`
  subcommand. Stored `counter.json` files are ignored and left untouched.

### Changed

- Core's `Clock::now` returns `Now`; counter timestamps keep their existing Unix
  millisecond format. Core rejects direct clock reads through jiff as well as `std`.

- `bunshin tui` now opens an empty-day shell with `q` and Ctrl+C to quit.
