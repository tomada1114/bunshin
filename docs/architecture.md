# Architecture

This page describes the layers every app cut from this template starts with, how they
talk, and what is contract. What an app decides on top of them — where it keeps state,
its dependencies, the platforms it targets, the permissions it asks for, whether it ever
ships releases — is recorded in the `deciding-architecture` skill's decision log. The
reasoning behind the layers themselves is the README's [Design
Philosophy](../README.md#design-philosophy).

## Layers

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ crates/bunshin  the `bunshin` binary: the tool and the composition root.         │
│ Arguments in; data on stdout, diagnostics on stderr, an exit code out.       │
│ It translates; it decides nothing.                                           │
└──────────────────────────────────────────────────────────────────────────────┘
      │ constructs the adapters, hands them to core
      ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ crates/bunshin-platform  adapters: the real OS and filesystem, behind ports    │
└──────────────────────────────────────────────────────────────────────────────┘
      │ implement core's ports; call core's use cases
      ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ crates/bunshin-core  rules, state, ports (traits), and the views and screens   │
│ the binary shows. No OS API, no terminal, no direct I/O. Builds on Linux.    │
│ Coverage floor: 80% of lines, 80% of functions.                              │
└──────────────────────────────────────────────────────────────────────────────┘
  crates/bunshin-test-support  fakes and one contract function per port
                             (a [dev-dependencies] entry only; never ships)
```

Dependencies point one way, toward core: `bunshin-platform` → core; `bunshin` (the binary)
→ platform and core. Core depends on neither, and platform does not know the binary
exists. Everything runs in one process: the subcommands and the full-screen view call
core directly, so there is no IPC layer and no serialized boundary between them.

### How the boundaries are enforced

| Layer | What fails |
|---|---|
| Compile time | `crates/bunshin-core/Cargo.toml` names no OS, terminal, or platform crate, so code in core cannot call one. |
| Dependency closure | A harness check (`just check-harness`) reads `cargo metadata` and fails if core's normal and build dependency closure contains a crate the boundary sentence in `AGENTS.md` › Architecture forbids — the macOS binding crates, the desktop-GUI crates, and `bunshin-platform` — or if a non-dev edge points at `bunshin-test-support`. `deny.toml`'s `[bans]` adds the direct-edge rule: `bunshin-platform` may be a direct dependency of `bunshin` only. |
| clippy in core | `crates/bunshin-core/clippy.toml` bans `print!`/`println!`/`eprint!`/`eprintln!`/`dbg!`, `std::io::{stdin, stdout, stderr}`, `std::fs::{File, OpenOptions, DirBuilder}` and every `std::fs` free function, `std::os::unix::fs::{symlink, chown, fchown, lchown, chroot}`, `std::path::Path`'s file-system queries (`exists`, `metadata`, `read_dir`, `is_file`, …), `std::net::{TcpStream, TcpListener, UdpSocket}`, `std::os::unix::net::{UnixStream, UnixListener, UnixDatagram}`, and `ToSocketAddrs::to_socket_addrs`, `std::process::{Command, exit, abort, id}`, `std::os::unix::process::parent_id`, `SystemTime::now`, `Instant::now`, both types' `elapsed`, `jiff::Timestamp::now`, `jiff::Zoned::now`, `std::env`'s argument, variable, and directory functions (including `current_exe` and `home_dir`), and `std::thread::{spawn, sleep, park_timeout, available_parallelism}` and `Builder::spawn`; `std::thread::scope` is allowed, since it joins its threads before it returns and so cannot outlive the call. `clippy::wildcard_enum_match_arm` is denied, so every `match` on a core enum names each variant. A ban whose path clippy cannot resolve would only warn and do nothing, so `just lint` and CI run clippy through `cargo xtask clippy-guard`, which fails with `ERR_CLIPPY_BAN_UNRESOLVED` instead. |

The forbidden-crate lists in `AGENTS.md`, the closure check, and `deny.toml` are kept
equal by a harness check. No gate stops core from naming clap, ratatui, or crossterm;
review holds that line, so a screen's state machine stays testable with plain values.

## Ports and adapters

Anything outside the process — the filesystem, the clock, and the on-device model —
reaches core through a port. Each port has the same four pieces:

| Piece | Where | `Clock` | `LanguageModel` |
|---|---|---|---|
| The port: a `Send + Sync` trait over types core owns | `crates/bunshin-core` | `time::Clock` | `model::LanguageModel` |
| The adapter: translates OS results into core's types and OS failures into core's error kinds, and decides nothing | `crates/bunshin-platform` | `SystemClock` | `FmLanguageModel` (macOS), `UnavailableLanguageModel` (Linux) |
| The fake: a real implementation answering from memory | `crates/bunshin-test-support` | `FixedClock` | `ScriptedLanguageModel` |
| The contract: the behaviour every implementation must have | `crates/bunshin-test-support` | `clock_contract` | `language_model_contract` |

`Clock::now` returns `Now` with both views of one millisecond-precision sample:
`instant: UnixMillis` for gaps and `local: jiff::civil::DateTime` for dates and
deadlines. `SystemClock` resolves the system zone, while `FixedClock` takes a fixed
offset so tests never depend on the host's zone. Core's pure `logical_date` function
uses the local view and `Tuning.day_boundary` (04:00 by default); a zone change cannot
alter the instant used to measure a gap.

`LanguageModel` probes availability and answers one `ModelRequest` with JSON text or
one typed `ModelError`. The macOS adapter passes the prompt unchanged on stdin and the
schema and instructions as literal arguments. It validates JSON syntax; schema and
proposal validation stay in core's caller. Every timeout or cancellation kills and
reaps the child, and pre-cancellation spawns nothing. Unknown process exits remain
`Failed`, with only the numeric code logged. `ModelRequest::new` and
`FmLanguageModel::with_tuning` use the shared `Tuning.model_timeout` (30 seconds by
default) for responses and availability probes respectively. The Linux adapter
reports `Unavailable(UnsupportedOs)`.

Core's `prompt` module builds a fresh chat request within a conservative estimate of
instructions, context JSON, schema, and answer reserve. It removes oldest chat before
closed tasks, then shortens open titles in request copies while retaining every open
identifier and time. Structural answer failures return `ModelError::Malformed` before
any proposal is applied. Domain-invalid proposals have typed refusals; valid proposals
share one visible change set and one undo entry. The complete reply is retained even
when it exceeds the length requested in the model instructions. The caller remains
responsible for persistence and for displaying the reply and refusals.

`prompt::checkin::build_checkin` uses the same assembly with a 300-token answer
reserve and mandatory compact trigger tuples. `checkin::calls::CheckinCalls`
queues one request behind owner conversation and rejects stale worker tokens.
Strict check-in parsing permits only silent, note or question; integer references
outside the task domain become general messages and planned looks are clamped.
Notes and questions require nonblank text; silent answers may carry empty text.
Delivery preserves tasks and undo, records suppression without updating the last
actual delivery time, and emits typed Bell/Save effects with one Bell per
unsuppressed row. Queued routine calls and retries recheck the same
active-hours, mute and minimum-gap guards as scheduling, including at worker
completion. A completed answer waits in memory until delivery is allowed, without
another model call; its trigger facts remain saved so a restart can reconsider
them. Both dispatch and completion receive current owner-queue and unsent-input
facts; a completed answer also waits until typing and owner work clear.
Enqueuing captures task facts when the scheduler batch is consumed. Before
dispatch, obsolete queued deadlines and their held facts are removed, including
when an unavailable model would otherwise produce a fixed fallback.
Same-day waiting events with the same delivery guards merge into one request,
retaining each event's enqueue facts and retry allowance. Opening-exempt events
stay separate from later routine events.
The scheduler marks an already mixed opening/routine batch as `GuardedOpen` or
`GuardedDayStart`: it remains one request and ordinary guards also apply at
completion. Pure opening batches retain their exemption.
Deadline events also recheck the current civil deadline before dispatch,
including after a restart; an obsolete
held event is removed without rewriting the once-per-task/kind fired record.
Before-deadline work expires once its deadline passes, both before dispatch and
when a completed answer is applied. An expired notice cannot suppress the next
tick's overdue notice or change the current planned look.
If a mixed batch retains a valid trigger, a general answer still applies to it;
only an answer referencing the obsolete task is discarded with that event.
After discarding a stale answer, surviving events return to held storage and
wait for the next actual tick, retaining their facts and retry allowances.
Renaming a queued task keeps its deadline event: dispatch uses the current title
and validates the deadline-defining facts rather than an unused earlier title.
A failed in-flight batch also joins eligible events queued
during its call; a retry waits for an actual tick, and fresh events retain their
own single retry even when sent alongside a previously failed event.
Task-specific replies are discarded when a task known at dispatch was closed,
deleted or edited during the call; discarding them leaves the current planned
look unchanged. Already closed task references remain valid when their
dispatch-time status and facts are unchanged; reopening invalidates the old response.
A task created during the call cannot capture a reference unknown at dispatch;
that reference remains general. Deadline fallbacks compare current task facts
with the dispatch snapshot too, so an edited time, kind or title cannot produce
a notice based on an obsolete deadline. Held facts remain saved throughout the
worker call and are removed only when its matching result is applied; a restart
can reconsider them. They also remain saved while guards prevent dispatch.
Queue preparation restores current-day worker/completed facts if a scheduler
release hands those same events back; the caller composes both transitions before
persisting the resulting day.
A previous-day flight occupies the
worker until its matching completion, which releases it without applying old data.
Deadline failures use `FixedDeadline` facts with a pure formatter supplied by the binary.
Consuming a deadline-only fallback preserves an existing planned look or schedules
the configured default when no look exists. Non-deadline failures
retry once at a scheduler tick after completion. Spending that retry preserves
an existing planned look or schedules the configured default when none remains.
Unavailable non-deadline events
remain held in day data until recovery. A caller marks `CallContext.is_tick` only
when the scheduler evaluates, rather than on every terminal poll.
The fixed deadline formatter replaces title control characters with spaces.

The compact check-in trigger codes are private prompt encoding: b (before), a
(after), p (planned), s (day start), e (evening), and c (catch-up). They retain all
one hundred before/after events in the maximum fifty-task batch. No stored format
or task transition changes. The binary's fixed sentences and note/question labels
live in `wording.rs`; its pure formatting entry points precede worker wiring and
terminal rendering in the dependent issues.

`crates/bunshin-core/tests/contracts.rs` runs each contract against the fake, on Linux,
inside the coverage floor. `crates/bunshin-platform/tests/contracts.rs` runs the same
function against the real adapter, on the Linux and macOS CI runners when it needs only
a filesystem (each test gets its own temporary directory). An adapter test that needs a
logged-in GUI session, a TCC grant, or the Keychain is marked
`#[ignore = "local machine: <what it needs>"]` and runs only in `just test-local`, which
a human starts. The genuine-model contract is ignored with reason
`local machine: fm with Apple Intelligence enabled`; routine tests run it against a
stub and never invoke the real model. Core's integration tests live in
`crates/bunshin-core/tests/`, never in its inline `#[cfg(test)]` modules, because there
test-support's types would come from a second copy of core.

Ports are **synchronous**. Core is plain functions and state, so nothing in it is
`async`, and the binary calls a port directly. A port that is inherently a stream is
modelled as a callback or a channel the binary drives, never as async trait methods.

Errors are one `thiserror` enum per port or core module, with a variant per failure
the caller can act on, without user data. For example, `day::DayError` distinguishes
a missing task from an invalid title. The binary owns each user-facing sentence in
`wording.rs` and matches each variant without a wildcard. An error leaving the process
as data serializes as a typed code. Adapters translate OS failures at the boundary.

`crates/bunshin/src/main.rs` is the composition root, the place that constructs real
adapters and hands them to core. It constructs `JsonFileDayStore`, `SystemClock` and
file logging for the task screen; model wiring belongs here rather than in core.

The platform crate and the binary are outside the coverage floor. That is a
constraint, not a licence: they translate, so they have no branch worth a numeric gate.
The moment one needs a decision, the decision moves into core behind the port.

## The binary

`crates/bunshin` builds `bunshin`, the tool itself. `just install-cli` installs it into
`~/.cargo/bin` with `cargo install --locked --path crates/bunshin`; that writes outside the
checkout, so it is a human's recipe. The command-line contract, which
`crates/bunshin/tests/cli.rs` runs against the built binary with a temporary `HOME`:

- **Streams.** Data goes to stdout; diagnostics go to
  stderr: `error: <wording>` for a failed action, `warning: <wording>` for a degraded run
  (logging unavailable), and, in a debug build, a copy of each log line.
- **Exit codes.** 0 on success (including `--help` and `--version`), 1 when the action
  failed (missing terminal or `HOME`, terminal I/O, locked or invalid day data), 2 on a
  usage error (clap's own code, with its message and usage on stderr).
- **`--version`** prints `bunshin <version>`, the workspace version from `Cargo.toml`'s
  `[workspace.package]`.

`bunshin instructions` reads `InstructionsState`: the complete selected text goes to
stdout unchanged, without adding a newline; its source, file path and owner-text
length go to stderr. Reads create no
files or logs. `bunshin instructions edit` initializes a missing `instructions.md`
with core's default, launches the selected owner editor, waits, and validates the
saved text. `VISUAL` precedes `EDITOR`; its shell text is trusted owner configuration,
while the file path is passed as `$1` instead of interpolated. The file remains 0600,
including when an editor replaces it. Missing, empty and over-limit reasons and the
600-character Unicode scalar bound live in core, behind `InstructionsSource`; the
file adapter and in-memory fake share `instructions_contract`. Read-only access
refuses non-regular/linked entries and modes other than 0700 for the application
directory and 0600 for the file; typed errors distinguish these from invalid UTF-8.
Only the explicit edit path repairs permissions, without rewriting existing text.
No-editor failure guidance uses symbolic `~/` or `$XDG_DATA_HOME` locations as in UX
flow C3, while the read-only source view displays the resolved path.

`bunshin tui` draws today's task screen with ratatui over its crossterm backend
(reached only as `ratatui::crossterm`). Core's `MainScreen` owns focus, task forms,
`ScreenKey` dispatch, save-result state and quit confirmation. Tab moves between input
and tasks; `q` quits from tasks and Ctrl+C from anywhere. The binary executes each
`Save` synchronously before reading another event, then reports its typed result to
core. A failure retains the day, retries on the next change, and asks once before
quit; a published replacement with unconfirmed durability has a distinct header and
confirmation. The screen shows an error notice while full chat and model interaction
are forthcoming.

Startup refuses with exit 1 unless stdin and stdout are both terminals, before any
file I/O. It then takes the writer lease and loads the logical day before entering
raw mode. Locked, unreadable and unsupported data are refused without overwriting
day bytes; stderr uses symbolic storage locations rather than private owner paths.
The lease remains alive until the terminal is restored. Startup and save effects are
tested headlessly with shared fakes and the real file adapter; the existing non-TTY
CLI test proves no-I/O refusal. Tests never manufacture a terminal to get past that
guard. Logging goes only to the file while the screen owns the terminal. Raw mode,
the main screen and cursor are restored on a normal exit, an error and a panic hook.
No check runs the loop itself; a human running `bunshin tui` is its test.

## Logging

The binary and `bunshin-platform` log through the `tracing` macros; `bunshin-core` has no
`tracing` dependency and logs nothing. Only the binary installs a subscriber
(`bunshin_platform::init_logging`). Its files go to `~/Library/Logs/io.github.tomada1114.bunshin/`
on macOS and `$XDG_STATE_HOME/bunshin/logs/` (default `~/.local/state/bunshin/logs/`) on
Linux, as `bunshin.YYYY-MM-DD.log`, one per day, and the newest 14 are kept; retention counts
every file in that directory whose name starts with the writer's prefix, so a second
writer gets its own directory. The writer is synchronous: the volume is low, and an
early `process::exit` would drop a background writer's last lines. A debug build of a
plain subcommand also writes to stderr; `bunshin tui` writes to the file only, since a
line on the terminal it owns would corrupt the frame. Logging is best effort: when the directory cannot be used, the binary
prints a warning and still runs the action. No log line carries user data. `just logs`
prints the newest file's last lines and exits.

## Where new code goes

| You are adding… | It goes in… | Tested by… |
|---|---|---|
| A rule, a state change, a view a front end shows | `crates/bunshin-core` | unit tests and `crates/bunshin-core/tests/` (coverage-gated, Linux) |
| Access to the OS or the filesystem | an adapter in `crates/bunshin-platform`, behind a port in core, with a fake and a contract function in `crates/bunshin-test-support` | the contract against the fake (core) and against the adapter (`just test-platform`, or `just test-local` when a human is needed) |
| A subcommand, or the wording for a new error variant | `crates/bunshin` (wording in `src/wording.rs`) | `crates/bunshin/tests/cli.rs` (the built binary, a temporary `HOME`) and a test per variant in `wording.rs` |
| A screen's state, an action, a key | core, beside the model it drives (`ShellScreen`, `ShellAction`, `ShellKey`) | core tests with keys and actions as values, over the fakes |
| How a screen is drawn | `crates/bunshin/src/tui/view.rs` | ratatui's `TestBackend` tests in the same file |
| The terminal loop, wiring | `crates/bunshin/src/tui/mod.rs`, `crates/bunshin/src/main.rs` (composition root) | no check runs the loop; a human runs `bunshin tui` |

## What is contract and what is private

Nothing here is published as a library, so the contract is what something outside a
change can observe: another crate, a machine that ran an earlier build, a script or a
scheduled job that runs `bunshin`, or the user. These are contract; everything else is
private.

| Contract | What depends on it | What changing it requires |
|---|---|---|
| **Core's public API** — public items reachable from `crates/bunshin-core/src/lib.rs`, including `day`'s transitions, `TaskView` and file DTOs, the shared `Tuning`, `LanguageModel`, `ModelRequest`, `ModelAnswer`, `Availability`, `UnavailableReason`, `ModelError`, `CancelFlag`, and the clock and shell APIs | `bunshin-platform`, `bunshin-test-support`, `bunshin`, and their tests | Update every caller in the same pull request; the compiler finds them. A new port is a recorded decision. |
| **The data and log locations** — the bundle identifier `io.github.tomada1114.bunshin` (`BUNDLE_IDENTIFIER` in `crates/bunshin-platform/src/paths.rs` and `bundle_id` in the justfile) and the XDG directory name `bunshin` (`XDG_APP_NAME`) | Where the tool's files are on a machine that ran it: on macOS `~/Library/Application Support/io.github.tomada1114.bunshin/` and `~/Library/Logs/io.github.tomada1114.bunshin/` (and any privacy grant, keyed by the identifier); on Linux `$XDG_DATA_HOME/bunshin/` and `$XDG_STATE_HOME/bunshin/logs/` | Fixed once the tool has run anywhere but your checkout: a new name leaves the user's data behind under the old one. Changing it is a human's decision, recorded as a decision (`deciding-architecture`); the bootstrap sets both once. |
| **On-disk file formats** — see below | Files already on a user's disk; `just logs` and anyone reading the logs | A new version still reads the old format: a format version and a migration, with a test that reads a sample of the previous format. |
| **The command line** — `bunshin tui`, `bunshin today [--json]`, `--help`, `--version`, what goes to stdout and what to stderr, and the exit codes (0 success, 1 the action failed, 2 a usage error) — see [The binary](#the-binary) | A person, a script, or a scheduled job that runs `bunshin` | Keep the old form working, or treat the change as breaking and say so in `CHANGELOG.md`. |

### Read-only task output

`bunshin today` reads the logical date once from `Clock`, then loads that day through
`DayStore` without taking a writer lease or initializing logging. It prints tasks in
pane order: number, status mark, time, full title. Untimed rows have a blank time field.
Control characters become spaces in plain output so a saved title cannot split a row
or issue terminal control sequences; full-width text remains unchanged.

`bunshin today --json` writes one object and a final newline:
`format` (currently 1), `date` (the logical date), and `tasks` (number, title, kind,
time, status). Time is an HH:MM string or null. JSON preserves the complete title.
This output is core's `TodayView`, independently versioned from day files; origins,
messages, triggers and other persisted bookkeeping stay outside the output contract.
A missing day is empty: plain output has no bytes, while JSON has an empty task array.
Unreadable or unsupported data returns one Japanese `error:` line and exit 1; files
remain unchanged. Failed stdout writes also return exit 1 without a panic.

### On-disk file formats

**Day files** have a format-one DTO in `day::file`; its storage adapter is separate.
The JSON object has `"format": 1` and camelCase fields for the logical date, task-number
high-water mark, tasks, messages, planned look, delivery and mute instants, fired and
held triggers, and yesterday's record. Dates use `YYYY-MM-DD`, task times use `HH:MM`,
the planned look uses a civil datetime, and elapsed-gap timestamps use Unix milliseconds.
The next task number survives deletion and undo; one less than it is the day's consumed
creation budget. The default limit of fifty creations therefore also survives deletion,
undo, and reload. Visible change and undo rows persist; the session's undo stack does not.
Loading requires current or historical task snapshots to account for every consumed
number from one through the high-water mark, so an inflated cursor cannot skip numbers.

A reader first decodes `FormatHeader` and checks it before decoding the rest of the
payload, so a newer format produces typed `NewerFormat` even when its shape has changed.
Format one ignores unknown fields and validates task invariants when converted to a
`Day`. No older day format has shipped: zero is explicitly unsupported, with no guessed
migration. A future format must define how to read format one before it can replace it.

Day files have no size cap. Whole-file reading and replacement can exhaust memory or
other resources for very large files. Saves sync a private temporary file, rename it,
then sync the containing directory. A failure before publication leaves the previous
file intact; `PublishedButNotDurable` means the complete replacement is already visible
but directory sync failed, so crash durability is unconfirmed. Its serialized error
code is `publishedButNotDurable`. The OS writer lease determines exclusion; its stored
PID is advisory and can be absent or stale before the new holder publishes it.

Legacy sample files are ignored and never deleted. The empty shell reads no stored
application state; future persistence adapters use the versioned day DTO above.

**Log files**: `bunshin.YYYY-MM-DD.log` from the binary in
`~/Library/Logs/io.github.tomada1114.bunshin/` on macOS and `$XDG_STATE_HOME/bunshin/logs/` on Linux,
dated in UTC, one per day, the newest 14 kept. Each line is `tracing-subscriber`'s plain
text format: an RFC 3339 timestamp, the level, the target, the message, and its fields.
The message wording is private.

**Private** is everything else: `pub(crate)` and private items, how an adapter talks to
the OS behind its port, the full-screen view's layout and styling, file and module
layout, test helpers, and log wording. Changing any of it needs only the gates that
already run.

No gate notices every broken contract item. The compiler guards core's public API, and
`crates/bunshin/tests/cli.rs` guards the command line; a changed file format or a renamed
identifier passes every check and fails on the user's machine, so review is what
catches it, and a user-visible change to any contract item owes a
`CHANGELOG.md` entry.
