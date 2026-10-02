# Bunshin's design as it stands

What the app is built as today, on top of the template's layers (`docs/architecture.md`).
The reasons and the rejected options are in [decision-log.md](decision-log.md); the
behavior each part serves is in `docs/product/requirements.md` (cited as §n). Values
marked † are starting values that live in core's `Tuning` and are tuned by use.

## Principles

Each one rules something out.

1. **The model proposes; core decides.** Every model answer is a typed proposal that core
   validates before anything changes (§3.3). No model output is applied as text, and a
   check-in never changes a task (§3.5).
2. **Core stays synchronous and deterministic.** No `async` runtime anywhere; time, files,
   the instructions, and the model reach core only through ports. A whole day, ticks and
   model answers included, replays in a test with a fixed clock and a scripted model.
3. **One writer.** Only `bunshin tui` writes the day files, and only one of it runs.
   The subcommands read. No merge logic exists, because no second writer does.
4. **Plain local files, owner only.** The day is a JSON file the owner can read; nothing
   is sent anywhere but to `fm` on the same Mac (§4, Non-goals). No database, no network
   client, no backup or export.
5. **Everything but the words works without the model.** Every key, the list, the
   inbox, saving, and the deadline notes' fixed sentences work when `fm` is missing,
   refuses, or is slow (§3.4, §3.5, §4).
6. **Budget before the call.** Core sizes every prompt to the 4,096-token window before
   it calls, with a conservative estimate, and never asks the model to count.

## Crates and ports

The template's four crates stay; no crate is added to the workspace.

| Crate | Holds |
|---|---|
| `bunshin-core` | the day (tasks, messages, inbox states, change sets and undo), the check-in rules (triggers, active hours, the gaps, holding and catch-up, mute), the day's rhythm (logical date, day start, leftovers, yesterday's record, evening review), the instructions' limit and default text, prompt assembly, the token estimate, the answer schemas and their parsing, the TUI screen's state and key table, `Tuning`, and the day file's versioned shape |
| `bunshin-platform` | the adapters below, the data and log directories, launching the owner's editor |
| `bunshin-test-support` | one fake and one `<port>_contract` function per port |
| `bunshin` | clap subcommands, the TUI loop, the model worker thread, the view, `wording.rs` |

Ports core declares (all synchronous `Send + Sync` traits):

| Port | Real adapter | Fake | What it fixes |
|---|---|---|---|
| `Clock` | `SystemClock` (jiff, system time zone) | `FixedClock`, settable | `now()` returns both the instant (`UnixMillis`) and the local civil date-time (`jiff::civil::DateTime`). Gaps and the 30-s timeout are measured on the instant, so a zone change cannot fake a sleep; triggers, the 04:00 boundary, and active hours use the civil time (§3.6 "Clock or time zone change") |
| `DayStore` | `JsonFileDayStore` | `InMemoryDayStore`, `FailingDayStore` | load a logical date's day, save a day whole, find the last day on record before a date, and take the single-writer lock |
| `InstructionsSource` | `FileInstructions` | `InMemoryInstructions` | read the owner's text (absent, empty, or text), write the default the first time, report the file's path for display |
| `LanguageModel` | `FmLanguageModel` (macOS), `UnavailableLanguageModel` (Linux) | `ScriptedLanguageModel` (queued answers and errors) | report availability; answer one `ModelRequest` (instructions text, prompt text, schema JSON, timeout) with the answer's JSON or a typed `ModelError`, stopping early when a shared cancel flag is set |

The counter sample (`CounterStore`, `JsonFileCounterStore`, the `counter` module) is
removed once the first real port lands.

## Dependencies

Runtime: the template's `clap`, `ratatui` (crossterm backend), `serde`, `serde_json`,
`thiserror`, `tracing`, `tracing-appender`, `tracing-subscriber`, plus **`jiff`**:

- in `bunshin-core` with `default-features = false, features = ["std"]` — civil
  date-times and arithmetic only;
- in `bunshin-platform` adding `tz-system` and `tzdb-zoneinfo`, so `SystemClock` resolves
  the system zone from `/etc/localtime` and `/usr/share/zoneinfo`.

jiff 0.2.37, `Unlicense OR MIT`, `rust-version` 1.70, published 2026-09-12
(https://crates.io/api/v1/crates/jiff, checked 2026-10-02); `TimeZone::system()` honors
`TZ` first and then reads `/etc/localtime`, and returns an unknown zone without
`tz-system` (https://docs.rs/jiff/latest/jiff/tz/struct.TimeZone.html, checked
2026-10-02). `TZ` is the OS's convention, not a Bunshin setting.

Core may not read the clock through jiff: `jiff::Timestamp::now` and `jiff::Zoned::now`
join the bans in `crates/bunshin-core/clippy.toml` in the same change that adds jiff to
core (a ban on an item clippy cannot resolve fails the clippy guard, so not before).

Nothing else: no async runtime, no HTTP client, no SQLite, no FFI binding.

## Data

- **Place:** the template's data directory — on macOS
  `~/Library/Application Support/io.github.tomada1114.bunshin/`, on Linux
  `$XDG_DATA_HOME/bunshin/` — outside any checkout, so no data file can reach git.
- **Layout:**
  - `days/YYYY-MM-DD.json` — one file per logical date (the day that starts at 04:00†).
  - `instructions.md` — the owner's instructions, UTF-8, ≤ 600 characters† (§3.8).
  - `tui.lock` — the single-writer lock.
- **Permissions:** the directory and `days/` are created `0700`, every file `0600`
  (`DirBuilderExt::mode`, `OpenOptionsExt::mode`); a directory found wider is narrowed at
  start.
- **The day file:** one JSON object with `"format": 1` and the Day of §5 — tasks,
  messages, unprompted messages with their inbox states, fired and held triggers, the
  next planned look, the last unprompted time, the mute, yesterday's record. Its shape
  is a versioned type in core (`day::file`, deriving serde), so the format is tested
  inside the floor; the adapter turns it into JSON with `serde_json`, which core uses
  only in tests. A file with a higher `format` is refused, never overwritten;
  an older one is migrated on read. The `--json` output of `bunshin today` is its own
  versioned view, not the file.
- **Writing:** the whole day on every change, to a temporary file in `days/`, flushed
  and `fsync`ed, then renamed over the old file — the same pattern as the template's
  `JsonFileCounterStore`. A reader such as `bunshin today` takes no lock and sees the
  old file or the new one, never half (§3.7). A day is a few tens of KB at most.
- **Failure:** a failed save keeps the day in memory, shows 「保存できません」, and
  retries on the next change; an unreadable day file stops the TUI with a message and is
  never overwritten (`docs/design/ux-guidelines.md`).
- **The lock:** `bunshin tui` opens `tui.lock` and takes `File::try_lock` for its whole
  life and writes its PID into the file; `WouldBlock` means another screen runs, and the
  refusal reads that PID for its message (§3.7, `docs/product/ux-flows.md` C4). The OS releases the lock when the
  file closes, crash included, so no stale lock is ever cleaned up
  (https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock, stable since 1.89,
  checked 2026-10-02).
- **Undo** (20† change sets) lives in memory for the session only.
- **No backup, export, or retention job:** every day is kept, and removing files is the
  owner's step (§3.7, Non-goals).

## Configuration

- No settings file. Every † value is a `Tuning` field.
- Environment: `VISUAL`, then `EDITOR`, read in `bunshin-platform` for
  `bunshin instructions edit` only. The editor runs through `/bin/sh -c '<value> "$1"'`
  so a value with flags (`code --wait`) works, and the path is passed as an argument,
  never spliced into the command. `HOME` and the `XDG_*` variables are read as the
  template already does. No other variable is read.

## The model call

- **How:** one child process per call: `fm respond --no-stream --schema '<schema JSON>'
  --instructions '<instructions>'`, with the prompt on stdin (`fm respond` reads its
  prompt from stdin, `fm respond --help`, observed 2026-10-02) so the day's tasks and chat
  never appear in another process's argument list. The instructions are on the command
  line for the call's duration; on a single-user Mac that is accepted. `fm` lives at
  `/usr/bin/fm`, and its only reference is `man fm` (observed 2026-10-02, macOS 27.0
  build 26A428).
- **Threads:** the TUI loop owns the terminal and core's screen. One model worker
  thread, started with the loop, runs one `LanguageModel` call at a time from a channel
  and sends the result back. Core keeps the queue and its order — an owner's message
  before a trigger (§3.5) — so the worker never decides.
- **Wait and cancel:** the adapter polls the child for exit; it kills and reaps it when
  the request's timeout (30 s†) passes or the cancel flag is set (Esc, quit). A killed
  call is `ModelError::Cancelled` or `ModelError::TimedOut`.
- **One call per event.** An owner's message gets one call whose schema answers
  `{changes: [...], reply}`; a check-in (a trigger batch, the day start, the evening
  review, a catch-up) gets one call whose schema answers
  `{kind: silent | note | question, task, message, next_look_minutes}` (§3.5). Core
  parses the JSON into typed proposals and validates them; an invalid change is dropped
  and said so (§3.3).
- **Budget.** The window is 4,096 tokens per session, and instructions, prompt, the
  schema, and the answer all count (https://developer.apple.com/documentation/technotes/tn3193-managing-the-on-device-foundation-model-s-context-window,
  checked 2026-10-02). Each `fm respond` is a fresh session, so every call carries its
  whole context. Core estimates tokens without the model — 1 per non-ASCII character,
  1 per 2 ASCII characters† — about twice the 0.5 token per Japanese character observed
  (2026-10-02). From the window it reserves the answer (450† chat, 300† check-in) and
  the schema's estimate, then fills in this order, each part only if it fits:
  instructions (owner's text, then the operating rules), the current time, yesterday's
  record (≤ 120†), today's open tasks, the last 5† unprompted messages' states, the
  triggers, closed tasks, and chat history newest first. Chat history and closed tasks
  are dropped before any open task (§3.1); if open tasks alone overflow, their titles
  are shortened, never dropped. `fm count-tokens` is not used: it took 1.97 s per call
  (observed 2026-10-02).
- **Availability:** checked at start and after a call fails as unavailable, then every
  10 min† until it returns (`docs/product/ux-flows.md` F10). `/usr/bin/fm` missing → not installed; `fm available` exit
  0 → available (observed 2026-10-02); exit 69 means the terms are not accepted
  (`man fm`), and the screen names `sudo fm license` for the owner to run, never the app
  (§4). On Linux the adapter is always "unavailable on this OS".
- **Errors** map to `ModelError` variants the screen can act on: `Unavailable(reason)`,
  `TimedOut`, `Cancelled`, `Refused`, `Malformed` (the answer failed the schema or the
  parse), `Failed` (any other exit). Exit 64 is a usage error and exit 1 an invalid
  schema (observed 2026-10-02) — both bugs, logged without the prompt. What a user
  message and a trigger do on each error is §3.5 and §4.
- **Tests:** no routine check needs the real `fm` — it fails inside a sandboxed agent's
  process (observed under Codex CLI's sandbox, 2026-10-02) and does not exist on a CI
  runner. The core and the binary are tested against `ScriptedLanguageModel`;
  `FmLanguageModel`'s contract run is `#[ignore = "local machine: fm with Apple
  Intelligence enabled"]` and runs only in `just test-local`, a human's recipe.

## Main flows

- **Open.** Take the lock (or exit with C4's 「すでに起動しています（PID …）」), read the instructions,
  load today's file and the last day on record, check availability, draw. A first open
  of the day runs the day start (§3.6); triggers that came due while closed go to the
  model as one catch-up (§3.7).
- **An owner's message.** Core appends it, queues a chat call with the assembled prompt,
  and the header shows thinking. The answer's changes are validated, applied as one
  change set, saved, and shown as a change line with the reply; an error is one line and
  nothing changes.
- **A key.** Core applies the change set, saves, redraws; no model involved (§3.4).
- **The tick.** The loop polls the terminal with a short timeout and hands core the
  clock's `now` at least once a second; core runs the check-in rules when 60 s† have
  passed. A gap over 5 min† since the last tick is a sleep and becomes one catch-up. No
  trigger → no call and no write.
- **Quit or Esc.** Esc cancels a running owner's call; quit cancels any call, waits for
  the worker to stop, restores the terminal, and releases the lock by exiting. A quit
  while saves are failing asks once.

## Language

- Every sentence a user reads — the screen, `--help`, errors, notices — is Japanese and
  lives in `crates/bunshin/src/wording.rs`. clap's help text is set from those constants
  (`#[command(about = ...)]`, `#[arg(help = ...)]`), not from doc comments, so `///`
  stays English.
- Code, comments, docs, commits, and pull requests stay English (`AGENTS.md` ›
  "Important Reminders" states the exception).
- The default instructions and the operating rules are model input: they live in
  core's prompt module, not in `wording.rs`.

## Color lock

The role table in `docs/design/design-direction.md` is the lock: the terminal's 16 named
colors only; text in red, blue, or magenta; green only on the focused border; reversed
for the selected row; no faint text, no background color, no RGB or 256-color value.
In code it is a set of `const` `Style`s beside the labels in the view, asserted by the
`TestBackend` view tests (`building-tuis`). Changing a row is a recorded decision.

## Platforms and distribution

- macOS 27 or later on Apple silicon is where the app is used. Linux builds and runs
  every test, with the model reported unavailable; everything else works there.
- No TCC permission, no `unsafe`, no distribution: installed from the checkout with
  `just install-cli` (a human's recipe). The bundle identifier and the XDG name stay
  `io.github.tomada1114.bunshin` and `bunshin`.

## Quality targets

| Target | Value | How it is checked |
|---|---|---|
| A key to its redrawn frame, save included | ≤ 100 ms | the human's run; core holds no I/O but the save |
| A tick with no trigger | no model call, no file write | core tests with `FixedClock` and the scripted model |
| Every change saved before the next event is handled | always | core screen tests assert the `Save` effect |
| A prompt over the budget | never sent | core tests over a 50-task day and a long chat |
| No user text in a log line | always | review; `designing-errors` |
| Data files | `0600`, directory `0700` | `just test-platform` against a scratch `HOME` |
| Core coverage | lines 80, functions 80 | `just test-core` |

## Open

- Unverified: the exit codes and stderr of `fm respond` for a runtime model error
  (window exceeded, guardrail, rate limit). Until `just test-local` observes them, they
  map to `Failed`, and the budget is the only guard against an overflow. Apple's error
  cases name `exceededContextWindowSize`, `guardrailViolation`, `refusal`,
  `rateLimited`, and `concurrentRequests`
  (https://developer.apple.com/documentation/foundationmodels/languagemodelsession/generationerror,
  checked 2026-10-02). Settled by the first local run of the adapter's contract.
- The language of the operating rules (Japanese or English prompt text). Settled by
  comparing both on real days; either way it is model input in core.
- Whether `--greedy` sampling suits the check-in call. Settled the same way; a `Tuning`
  switch until then.
