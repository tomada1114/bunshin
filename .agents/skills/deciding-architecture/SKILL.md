---
name: deciding-architecture
description: >
  Holds Bunshin's architecture and design policy as they stand (the principles, how the
  domains map onto crates and ports, the day files and the instructions file, how the fm
  model is called and budgeted, the worker thread and the tick, the TUI colors, the
  quality targets) and the dated log of every architecture decision with the options it
  rejected. Use when a change adds a crate or a port in bunshin-core, changes persistence
  or configuration, adds a crate dependency, adds or drops a target platform, adds
  distribution, raises rust-version, needs a TCC permission, lifts unsafe_code =
  "forbid", changes the bundle identifier or XDG directory name, adds a language or a TUI
  color, or replaces clap or ratatui; when deciding how a feature calls the model, stores
  data, or reads the clock; when proposing or revising a decision; or when asked why the
  app is built this way.
---

# Deciding Architecture

**Owns:** Bunshin's architecture and design policy as they stand now, the dated log of
the decisions behind them, and how a decision is proposed, accepted, and revised.
**Does not own:** the layers every app cut from the template starts with
(`docs/architecture.md`, README's Design Philosophy); what the app does and its
non-goals (`managing-the-product`); the order of outcomes (`steering-the-roadmap`); how
a crate is reviewed and declared (`managing-dependencies`); a gate's configuration
(`changing-gates`); which other document a change lands on (`updating-docs`).

## Where things are

- [references/design.md](references/design.md) — the design as it stands: principles,
  crates and ports, data, the model call, the main flows, the color lock, the quality
  targets, and what is still open. Edited in place; it never says "used to".
- [references/decision-log.md](references/decision-log.md) — one dated entry per
  decision: what was decided, the options rejected and why, and the sources. Entries
  are appended, never rewritten.
- This file — when a change owes a recorded decision, and how one is made.

There are no numbers, statuses, or index. A decision is either in the log, accepted by
the owner, or it is a proposal in a conversation or a pull request and nowhere else.

Read `design.md` before shaping any feature that touches the model, the day files, the
clock, the TUI loop, or a dependency. **REQUIRED:** read the decision-log entry behind a
rule before arguing against it; the rejected option you are about to propose may
already be there with its reason.

## When a change owes a recorded decision

A change owes one when it is expensive to reverse or when later work will build on it.
`AGENTS.md` › "Before changing the architecture" is the template's list; why each is
expensive, and what Bunshin adds:

- **A new crate in the workspace, or a new port in core.** A crate fixes a dependency
  direction the boundary checks must learn; a port fixes the contract its adapter, its
  fake, its contract function, and every caller are written against.
- **Persistence or configuration** — the data directory, the day file's format or its
  `format` version, the instructions file, a lock, a settings file, or an environment
  variable the tool reads. Stored data outlives the code that wrote it; a setting is an
  interface the owner's habits come to depend on.
- **A new crate dependency**, runtime or dev. The review `managing-dependencies` asks
  for is the evidence the entry cites; the owner's sign-off is separate and still owed.
- **A target platform** added or dropped; **distribution** of any kind;
  **`rust-version`**; **a TCC permission**; **`unsafe`**; **the bundle identifier or the
  XDG directory name**; **replacing clap or ratatui** or moving either to a new major.
  Each changes what CI builds, what the owner installs, or what every screen and
  subcommand is written against.
- **How the model is reached** — the `fm` mode (`respond` per call), the number of calls
  per event, the answer schemas' shape, the token budget's rule, the timeout and cancel
  path. Every chat and check-in feature is written against these.
- **The thread model** — anything beyond the TUI loop plus one model worker, or any
  `async` runtime.
- **A language** — the user-facing wording is Japanese only; a second language, or
  moving the model-facing prompt text between languages.
- **The color lock** — any row of the role table in `docs/design/design-direction.md`,
  or a color beyond the terminal's 16 named colors.

A refactor inside a module, a test, a screen drawn with the locked styles, a rename that
crosses no boundary, a † starting value tuned in `Tuning`, or a fix that restores what
`design.md` already says owes none. Saying so in the pull request is a legitimate
outcome. The test: if a reader a year from now would ask "why is it like this?" and the
code cannot answer, the answer belongs in the log.

## How a decision is made

1. **Propose.** Name the decision, then 2–4 options, each with what it costs, and the
   line of `docs/product/requirements.md` (a section, a non-goal) each serves or
   violates. Recommend one when there is a real recommendation. Ask in the conversation
   (an option question to the owner) or in the pull request description; a proposal is
   never written into `design.md` or the log.
2. **Owner accepts.** Only the owner chooses. Their explicit choice in the session or a
   review comment is the acceptance; an agent never accepts its own proposal.
3. **Record in the same change as the code that follows it.** Edit `design.md` in place
   so it describes the result, and append one entry to the log. When the decision moves
   a template boundary, `docs/architecture.md` and `AGENTS.md` › Architecture are
   updated in the same pull request (`updating-docs`).
4. **Grant nothing.** A recorded decision explains; it does not authorize. A new crate,
   a release pipeline, or a permission grant still needs the sign-off `AGENTS.md` ›
   "Security and human approval" asks for, and a gate change still goes through
   `changing-gates`.

A decision the owner defers stays out of the log; `design.md` lists it under "Open" with
what would settle it.

## Revising a decision

- Edit `design.md` so it states the new design, and append a new log entry that names
  the earlier one by its date and title ("Revises 2026-10-02, Day files"). The earlier
  entry stays as written: it is the reasoning that held at the time.
- A small correction to an entry — a re-checked fact, a source that moved — is a dated
  line appended under it (`Corrected 2026-11-03: …`), never a silent edit.
- When `design.md` and the code disagree, neither is quietly edited to match: the next
  change moves the code toward the design, or a new decision changes the design.

## Facts

Keep four kinds of statement apart, in both reference files:

- **Verified** — a primary-source URL and `checked YYYY-MM-DD`, and the author opened
  the page that day: docs.rs and crates.io for a crate, the Rust and Cargo books, Apple's
  developer documentation, `man fm` for the `fm` command (Apple publishes no web page
  for it).
- **Observed** — what a command showed on the owner's Mac: "observed with `<command>`,
  YYYY-MM-DD", with the OS build when it matters.
- **Decided** — with its reason and the options it beat.
- **Unverified** — prefixed `Unverified:` and listed under `design.md` › Open, until it
  is verified or the claim that needed it is deleted.

Facts here move: what `fm` accepts, its exit codes, the model's window, a crate's
features. One confidently wrong fact costs the credibility of every correct one beside
it.

## Hygiene and checks

- Nothing in either file carries a secret, a personal path under a home directory, the
  owner's tasks or chat, or a reference to this repository's issues or pull requests
  (`just check-harness` fails on one in a skill). Call the person who decides "the
  owner".
- English, wrapped at the width the file already uses. Name a type, a crate, or a
  config key rather than a `path:line`, which rots with the next edit.
- After editing: `just agents-sync`, then `just agents-check`, `just check-harness`, and
  `mise exec -- typos .agents/skills/deciding-architecture`.
