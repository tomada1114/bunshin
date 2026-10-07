# Decision log

One entry per architecture decision the owner accepted, oldest first. Entries are
appended and never rewritten; a revision is a new entry that names the one it revises,
and a correction is a dated line under the entry. The design these add up to is
[design.md](design.md).

Entry shape: `### YYYY-MM-DD — Title`, then **Decided**, **Rejected** (each option with
why), and **Sources** where an external fact carried weight.

### 2026-10-02 — Decisions live in a skill, not in ADR files

- **Decided:** the architecture and its decisions live in this skill: the design as it
  stands, edited in place, and this log. No numbers, statuses, or index. The template's
  `recording-architecture-decisions`, `docs/architecture/README.md`, and
  `docs/architecture/adr/` are removed; `docs/architecture/roadmap.md` stays.
- **Rejected:** the template's ADR tree — the owner found numbered files with statuses
  more to manage than one person's app needs; ADR files without statuses — still one
  file per decision to find and keep consistent, where an agent needs the current design
  in one place.

### 2026-10-02 — A product skill in front of the issue tracker

- **Decided:** `managing-the-product` holds the feature map and the intake every feature
  request goes through before an issue is filed. The requirements stay in
  `docs/product/`.
- **Rejected:** moving the requirements into the skill — they are the product's
  document, cited by issues and by `AGENTS.md`'s Product section, and a skill is the
  procedure around them; no intake — a request filed straight as an issue skips the
  non-goals and leaves the requirements stale.

### 2026-10-02 — No async runtime; one model worker thread

- **Decided:** the TUI loop, one model worker thread, and channels between them. Core
  stays synchronous; the worker runs one call at a time; Esc and quit kill the child
  process.
- **Rejected:** an async runtime (tokio) — the only concurrent work is one child process
  at a time (§3.1 "One call at a time"), and async would reach core's ports, which the
  template keeps synchronous; a thread per call — two calls would race for the model
  and the order rule of §3.5.

### 2026-10-02 — Ports: LanguageModel, DayStore, InstructionsSource; Clock gives local time

- **Decided:** three new ports, and `Clock::now` widened to return the instant and the
  local civil date-time.
- **Rejected:** one storage port for days and instructions — they differ in writer (the
  TUI versus the owner's editor), format, and failure handling; the binary reading the
  instructions file and passing text to core — the default, the limit, and the "why the
  default is in use" view are decisions core must test; the clock returning only an
  instant with core converting — conversion needs the system zone, which is I/O.
- Superseded in part on 2026-10-07: the storage and instructions ports are retired; Clock and LanguageModel remain.

### 2026-10-02 — The model is reached by one `fm respond` process per call

- **Decided:** `fm respond --no-stream --schema '<JSON>' --instructions '<text>'` per
  call, the prompt on stdin, killed on timeout or cancel.
- **Rejected:** `fm serve` kept running — it needs an HTTP client crate, a server's
  start, stop, and health handling, and whether its Chat Completions API honors a schema
  is undocumented; an FFI binding crate (fm-rs and others) — its build script needs
  Swift and Xcode, so the Linux CI job could not build the platform crate without
  `cfg` work, for a gain of about a second per call.
- **Sources:** `man fm` and `fm respond --help` (observed 2026-10-02, macOS 27.0 build
  26A428): `--schema` takes a file or inline JSON, the prompt may come on stdin, exit 69
  means the terms are not accepted. A schema-constrained `fm respond` took 1.26–3.58 s
  (observed 2026-10-02).

### 2026-10-02 — One schema-constrained call per event, budgeted by estimate

- **Decided:** an owner's message and a check-in each get exactly one call with a JSON
  schema; core sizes the prompt with a conservative character-based estimate and drops
  chat history before any open task.
- **Rejected:** two calls for a check-in, "whether to speak" then "what to say" — about
  twice the wait, and with one call at a time an owner's message would queue behind
  both; measuring each prompt with `fm count-tokens` — 1.97 s per call (observed
  2026-10-02), as long as the answer itself.
- **Sources:** the 4,096-token session window counts instructions, prompts, schemas, and
  responses
  (https://developer.apple.com/documentation/technotes/tn3193-managing-the-on-device-foundation-model-s-context-window,
  checked 2026-10-02). Free-form "should I speak now?" was echoed back, while the same
  question with a schema gave a usable decision (observed 2026-10-02).
- Superseded on 2026-10-07: character turns use one {body} call with a fixed board context instead of chat and check-in events.

### 2026-10-02 — One JSON file per day, rewritten whole

- **Decided:** `days/YYYY-MM-DD.json` with a `format` version, written to a temporary
  file, `fsync`ed, and renamed over the old one on every change. The data directory sits
  outside any checkout; there is no backup, export, or `.gitignore` entry for it.
- **Rejected:** an append-only JSONL log replayed on read — a reader can meet a
  half-written last line (§3.7 "written whole or not at all"), and undo and inbox state
  would need replay logic; SQLite — a C-building dependency, and the owner could no
  longer read the day as a plain file (Non-goals: "the day files are plain local files");
  a backup now — the owner chose none for the first version (2026-10-02).
- Superseded on 2026-10-07 by the in-memory board; no board file is written.

### 2026-10-02 — Data directory, owner-only files, a lock file

- **Decided:** the template's data directory; directories `0700`, files `0600`; the
  instructions in `instructions.txt` beside `days/`; a second `bunshin tui` is refused
  by `File::try_lock` on `tui.lock`.
- **Rejected:** the instructions in a separate config directory — a second place and a
  second permission rule for one small file whose path `bunshin instructions` prints
  anyway; a PID file — it goes stale after a crash and needs cleanup logic the OS lock
  does not.
- **Sources:** `File::try_lock`, stable since Rust 1.89, released when the file closes
  (https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock, checked
  2026-10-02); the workspace's `rust-version` is 1.90.
- Corrected 2026-10-02: the file is `instructions.md`, as `docs/product/ux-flows.md`
  T5, C2, and C3 show; the lock file also holds the running screen's PID, which C4's
  refusal prints.
- Superseded on 2026-10-07: the board has no user-data directory or single-writer lock, and the instructions file is retired.

### 2026-10-02 — Configuration: VISUAL and EDITOR only

- **Decided:** no settings file; the † values live in `Tuning`. The tool reads `VISUAL`,
  then `EDITOR`, for `bunshin instructions edit`, besides the `HOME` and `XDG_*`
  variables the template reads.
- **Rejected:** a settings file now — it is a Later item ("Settings for the starting
  values"), and values tuned by rebuild suit one owner until one changes often.
- Superseded in part on 2026-10-07: the owner editor variables are no longer read; HOME and XDG still locate logs.

### 2026-10-02 — Time through jiff

- **Decided:** jiff in core (civil types only) and in platform (the system zone). Core
  is barred from jiff's clock reads by `clippy.toml`. The owner's choice of this option
  on 2026-10-02 is the dependency sign-off.
- **Rejected:** chrono in platform only — chrono pulls `core-foundation-sys` on macOS,
  so it cannot enter core's dependency closure, and core would hand-write date
  arithmetic (month ends, leap years) for the 04:00 boundary; no crate — the local
  offset would need `libc` (`unsafe`) or spawning `date`.
- **Sources:** jiff 0.2.37, `Unlicense OR MIT`, `rust-version` 1.70
  (https://crates.io/api/v1/crates/jiff, checked 2026-10-02); `TimeZone::system()`
  (https://docs.rs/jiff/latest/jiff/tz/struct.TimeZone.html, checked 2026-10-02). Both
  licences are in `deny.toml`'s allow list.
- Superseded in part on 2026-10-07: the 04:00 day boundary is retired; Clock still supplies local display time and an instant.

### 2026-10-02 — Japanese user-facing wording, an exception to English

- **Decided:** every user-facing sentence, `--help` included, is Japanese, in
  `wording.rs`; clap's help comes from those constants, so doc comments stay English.
  Code, docs, commits, and pull requests stay English; `AGENTS.md` states the exception.
- **Rejected:** English wording — the owner reads and speaks to the secretary in
  Japanese (§4 "Language"); both languages — a second language is a non-goal for one
  owner and doubles every wording test.

### 2026-10-02 — The color lock is design-direction.md's role table

- **Decided:** the role table in `docs/design/design-direction.md`, as confirmed by the
  owner, is the lock: the terminal's 16 named colors, text only in red, blue, and
  magenta, green only on the focused border, reversed selection, no faint text.
- **Rejected:** as that document records — monochrome with modifiers only, and a fixed
  RGB palette.

### 2026-10-02 — Platforms: macOS in use, Linux builds; no TCC, no distribution

- **Decided:** macOS 27 on Apple silicon is the target; Linux keeps building and testing
  with the model reported unavailable. No TCC permission and no distribution: installed
  from the checkout.
- **Rejected:** dropping Linux — the template's CI runs core and the binary there, and
  the only cost is one always-unavailable adapter.

### 2026-10-02 — The real model is tested only on the owner's Mac

- **Decided:** every routine check uses `ScriptedLanguageModel`; the real adapter's
  contract is `#[ignore = "local machine: ..."]` and runs in `just test-local`.
- **Rejected:** calling `fm` from `just check` — it fails inside a sandboxed agent's
  process (`ModelManagerError 1008`, observed under Codex CLI's sandbox, 2026-10-02),
  does not exist on a CI runner, and is slow and nondeterministic.

### 2026-10-02 — The template sample goes with the first real port

- **Decided:** the template's sample (its port, adapter, fake, contract, and
  subcommand) is removed by its own unit of work, before or with the first port.
- **Rejected:** keeping it as a worked example — `design.md` and the real ports replace
  it, and a sample with no product meaning misleads the next reader.

### 2026-10-03 — Lock PID is advisory

- **Decided:** retain OS lock acquisition followed by PID publication. The owner
  chose to document that a contending screen can see an absent or previous PID in
  that interval. Only the OS lock guarantees exclusion; the displayed PID is a hint.
- **Rejected:** changing the publication protocol to promise an exact holder PID —
  unnecessary complexity for advisory startup guidance when the OS already prevents
  two cooperating writers.
- Superseded on 2026-10-07: the board does not take a writer lock.

### 2026-10-03 — Day files have no size cap

- **Decided:** retain whole-file JSON reads and replacements without a size limit.
  The owner chose to document that very large files can exhaust memory or other
  resources; no maximum is introduced in `Tuning`.
- **Rejected:** a 16 MiB maximum that refuses reads and saves while preserving the
  file — changes the existing unlimited contract and adds a new rejection policy
  the owner does not want in this version.
- Superseded on 2026-10-07: the prototype has no user-data file.

### 2026-10-03 — Distinguish publication from confirmed durability

- **Decided:** sync the parent directory after rename. If that sync fails, return
  `PublishedButNotDurable`: the new complete data is already visible, but its crash
  durability is unconfirmed. Keep `Unavailable` for failures before publication,
  where the old file remains intact. The owner explicitly chose this distinction.
- **Rejected:** reporting the post-rename failure as an ordinary failed write —
  falsely implies that the old file remains and can mislead retry and quit handling;
  treating the failure as success — hides the unconfirmed durability.
- Superseded on 2026-10-07: there is no persisted board state to publish.

### 2026-10-03 — Reuse existing serde_json in core prompt handling

- **Decided:** reuse the workspace's existing `serde_json` as a normal dependency
  of core for structured prompt context and strict answer-envelope parsing. The
  owner explicitly accepted this reuse on 2026-10-03; the version, features and
  already-shipped packages stay unchanged.
- **Rejected:** a hand-written JSON encoder and parser — duplicates escaping,
  Unicode handling and strict validation already supplied by the existing crate;
  parsing domain proposals in platform — moves core decisions outside its tests.

### 2026-10-03 — Reuse existing serde_json for the public task output

- **Decided:** the binary directly reuses the workspace's existing `serde_json`
  to serialize the independently versioned core task view. The owner explicitly
  accepted this reuse on 2026-10-03; no package, version or feature is introduced.
- **Rejected:** serializing the persisted day object — exposes bookkeeping and
  couples script output to storage; hand-written JSON formatting — duplicates
  escaping and risks invalid output for complete owner titles.
- Superseded on 2026-10-07: the read-only task command and its JSON output are retired.

### 2026-10-03 — Reuse shared port fakes in binary tests

- **Decided:** the binary takes the existing workspace `bunshin-test-support`
  as a dev dependency for TUI and CLI tests against the shared port fakes. The
  owner explicitly accepted this reuse on 2026-10-03. It does not ship.
- **Rejected:** duplicating fakes inside the binary — separates them from the
  shared port contracts and creates a second implementation to keep consistent.

### 2026-10-06 — Day file format 2 records each trigger's spent retry

- **Decided:** bump the day file to `"format": 2` and add `retryingTriggers`, always a
  subset of `heldTriggers`, persisting the check-in queue's in-memory retry sets. The
  queue restores a recorded trigger as a next-tick retry, so restarting Bunshin
  neither grants a failed non-deadline trigger another retry nor spends one (§3.5).
  Format 1 is migrated on read with no trigger retrying, so no retry counts as spent;
  a binary that knows only format 1 refuses a format-2 file as `NewerFormat` and never
  overwrites it; a retrying trigger that is not held is invalid data. The
  `bunshin today --json` output is versioned separately and unchanged. The owner
  accepted this on 2026-10-06.
- **Rejected:** an optional field in format 1 — serde ignores unknown fields, so an
  older binary would read the file and silently drop the retry record on its next
  save; treating every restored held non-deadline trigger as already retrying —
  `heldTriggers` also holds triggers that only waited on the guards (active hours,
  the mute, the delivery gap) and never reached the model, which would lose the
  "retried once" allowance of §3.5.
- Superseded on 2026-10-07: the board has no day file or persisted check-in retry state.

### 2026-10-07 — The board is in memory only

- **Revises:** 2026-10-02, One JSON file per day, rewritten whole; 2026-10-02, Data directory, owner-only files, a lock file; and 2026-10-06, Day file format 2 records each trigger's spent retry.

- **Decided:** one shared board exists in memory, holds at most 200 posts, and is lost on
  quit. The app does not read or delete earlier day data and takes no day-store lock.
- **Rejected:** keeping a board file — a prototype needs no user-data format or persistence
  behavior to prove its conversation loop.

### 2026-10-07 — Core chooses board turns with a seeded generator

- **Revises:** 2026-10-02, One schema-constrained call per event, budgeted by estimate; core now chooses the speaker, kind, and topic before the model call.

- **Decided:** core chooses the speaker, post kind, and new-topic values with a small
  xorshift generator seeded once from the clock's instant. The model writes only the
  post body.
- **Rejected:** adding the rand crate — it adds a dependency and a review/sign-off step;
  letting the model choose the speaker and kind — the small on-device model handles that
  poorly and would need a richer schema.

### 2026-10-07 — One schema-constrained call per post

- **Revises:** 2026-10-02, One schema-constrained call per event, budgeted by estimate; each scheduled character post now has its own fixed board context.

- **Decided:** one fm respond call writes each character post using a schema with one
  string field, body. The prompt includes the latest 12 posts† and the selected task;
  keep the existing 30-second timeout. The instruction asks for at most 80 characters,
  and display trims to 120† characters.
- **Rejected:** a separate call to decide whether to speak — it doubles latency and
  complicates the one-worker rule; free-form output — a schema constrains the response
  to one body field.
- **Observed:** the owner received a real schema-constrained reply in bunshin tui,
  2026-10-07.

### 2026-10-07 — Character and topic instructions are fixed in core

- **Revises:** 2026-10-02, Ports: LanguageModel, DayStore, InstructionsSource; Clock gives local time; and 2026-10-02, Configuration: VISUAL and EDITOR only.

- **Decided:** character personas and topic values are compiled-in prompt data. There is
  no owner instructions file or editor command.
- **Rejected:** keeping an editable instructions file — the prototype tests fixed
  characters, and the file adds persistence, validation, and editing behavior.

### 2026-10-07 — Retiring the secretary contracts is breaking

- **Revises:** 2026-10-02, Data directory, owner-only files, a lock file; 2026-10-03, Reuse serde_json for the public task output; and 2026-10-06, Day file format 2 records each trigger's spent retry.

- **Decided:** the only subcommand is tui; the old task-reading commands and day-file
  format are retired. Earlier data is left untouched and unread. Implementation changes
  record their visible removals in the Unreleased changelog.
- **Rejected:** retaining the old commands or reading old day files — compatibility
  requires the secretary's data model and widens a prototype that is meant to test one
  board conversation loop.

### 2026-10-07 — The reduced interim day format writes version 3

- **Revises:** 2026-10-06, Day file format 2 records each trigger's spent retry; and
  2026-10-07, Retiring the secretary contracts is breaking.

- **Decided:** while the transitional task and chat screen still reads day files, keep
  format 1 and format 2 readable, then write the reduced day payload as format 3. A
  format-2 binary refuses the reduced payload as newer instead of treating it as the
  same schema. This compatibility bridge ends when the task screen and day store are
  removed; the board remains in memory only.
- **Rejected:** continuing to write format 2 — its reader requires retryingTriggers,
  which the reduced model no longer writes, so the same version would describe two
  incompatible schemas.
