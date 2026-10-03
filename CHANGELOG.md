# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Bounded owner-chat requests keep every open task within the estimated model
  window. Valid model proposals share one undoable change set; invalid proposals
  have typed refusals, and replies remain whole. The core API is ready for the
  terminal chat flow.

- A responsive main task screen with direct task keys, forms and help. Each change
  is saved before the next input; failed saves retain the day and retry on the next
  change. Startup refuses locked or invalid data before entering the terminal;
  quitting distinguishes unsaved changes from unconfirmed crash durability.

- `bunshin instructions` shows the text in use and its source; `bunshin instructions
  edit` initializes an owner-only file and opens `VISUAL`, then `EDITOR`. Missing,
  empty or over-600-character text uses the shipped default without truncating the
  owner's file. Saved text is checked before the next call uses it.

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
