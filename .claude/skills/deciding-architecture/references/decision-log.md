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

### 2026-10-02 — One JSON file per day, rewritten whole

- **Decided:** `days/YYYY-MM-DD.json` with a `format` version, written to a temporary
  file, `fsync`ed, and renamed over the old one on every change. The data directory sits
  outside any checkout; there is no backup, export, or `.gitignore` entry for it.
- **Rejected:** an append-only JSONL log replayed on read — a reader can meet a
  half-written last line (§3.7 "written whole or not at all"), and undo and inbox state
  would need replay logic; SQLite — a C-building dependency, and the owner could no
  longer read the day as a plain file (Non-goals: "the day files are plain local files");
  a backup now — the owner chose none for the first version (2026-10-02).

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

### 2026-10-02 — Configuration: VISUAL and EDITOR only

- **Decided:** no settings file; the † values live in `Tuning`. The tool reads `VISUAL`,
  then `EDITOR`, for `bunshin instructions edit`, besides the `HOME` and `XDG_*`
  variables the template reads.
- **Rejected:** a settings file now — it is a Later item ("Settings for the starting
  values"), and values tuned by rebuild suit one owner until one changes often.

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

### 2026-10-03 — Day files have no size cap

- **Decided:** retain whole-file JSON reads and replacements without a size limit.
  The owner chose to document that very large files can exhaust memory or other
  resources; no maximum is introduced in `Tuning`.
- **Rejected:** a 16 MiB maximum that refuses reads and saves while preserving the
  file — changes the existing unlimited contract and adds a new rejection policy
  the owner does not want in this version.

### 2026-10-03 — Distinguish publication from confirmed durability

- **Decided:** sync the parent directory after rename. If that sync fails, return
  `PublishedButNotDurable`: the new complete data is already visible, but its crash
  durability is unconfirmed. Keep `Unavailable` for failures before publication,
  where the old file remains intact. The owner explicitly chose this distinction.
- **Rejected:** reporting the post-rename failure as an ordinary failed write —
  falsely implies that the old file remains and can mislead retry and quit handling;
  treating the failure as success — hides the unconfirmed durability.

### 2026-10-03 — Reuse shared port fakes in binary tests

- **Decided:** the binary takes the existing workspace `bunshin-test-support`
  as a dev dependency for TUI and CLI tests against the shared port fakes. The
  owner explicitly accepted this reuse on 2026-10-03. It does not ship.
- **Rejected:** duplicating fakes inside the binary — separates them from the
  shared port contracts and creates a second implementation to keep consistent.
