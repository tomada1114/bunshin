# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Core day rhythm: the day start at the first open of a logical day, or at the first
  key after 04:00 with the screen open, writes yesterday's record (counts and up to three
  titles each, within 120 tokens) that every model call carries, drops the previous
  day's held triggers, and queues a day-start check-in exempt from the delivery guards.
  The leftovers block offers the last day on record's open deadline and untimed tasks;
  `c`/`d`/`C`/`D` or a chat proposal carries them over as new untimed tasks or drops
  them, saving both days as one undoable change. The 18:00 evening review is queued
  once after the day start is settled, under every delivery guard. The terminal
  drawing and check-in dispatch are ready for the TUI integration.

- TUI chat accepts bounded Unicode input, queues owner messages, shows replies and
  undoable changes, and supports cancellation without blocking task keys. Model
  availability, recovery notices and a read-only instructions view remain visible.

- Core inbox reactions track answers, acknowledgements, dismissals, closed tasks
  and recent mute reactions in existing day files. Open notes and questions stay
  available when the model is unavailable; recent reactions accompany chat and
  check-in prompts. Inbox views and reply routing are ready for TUI integration.

- Core check-in calls retain a ready batch in one bounded request, deliver notes
  or questions with typed Bell/Save effects, and preserve tasks and undo. Deadline
  failures have fixed notes; other failures retry once and unavailable events wait.
  Same-task suppression and strict reply parsing precede the terminal integration.

- Bounded owner-chat requests keep every open task within the estimated model
  window. Valid model proposals share one undoable change set; invalid proposals
  have typed refusals, and replies remain whole. The core API is ready for the
  terminal chat flow.

- `bunshin today` reads the logical day's tasks without a writer lock; `--json`
  prints a stable versioned view. Missing days are empty, data errors preserve the
  files, and failed stdout writes return a runtime error.

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
