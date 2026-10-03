---
name: running-the-app
description: >
  Covers seeing a change work in the real bunshin binary without taking over the
  developer's machine or terminal: just test-platform and just logs as an agent's own
  evidence; cargo run --locked -p bunshin -- <subcommand> for a read-only run, and the
  built binary against a scratch HOME for anything that writes; the log line a debug
  build echoes to stderr; reading the daily log files (~/Library/Logs on macOS,
  $XDG_STATE_HOME/bunshin/logs on Linux); bunshin tui run by the human, never by an agent,
  since it takes over the terminal; what to ask a human for, once; and the evidence a pull
  request carries for behavior no gate asserts. Use when asked to run, launch, start,
  try, or look at the tool or its TUI, when a change must be verified in the running
  binary rather than in tests, when a log line is the only observable, or when deciding
  what to paste into a pull request.
---

# Running the App

**Owns:** getting evidence from the built binary: what an agent runs on its own, what it
asks a human to run, and what the pull request carries. **Does not own:** whether a
behavior belongs in a test instead (`placing-tests`, `tdd`); the command line's
contract (`designing-clis`); the terminal loop (`building-tuis`); an OS integration
behind a port (`integrating-system-apis`); the pull request itself (`create-pr`).

Running the tool proves wiring, not logic. Every decision is core's and gated by
`just test`; the subcommands are tested end to end by `crates/bunshin/tests/cli.rs`, and
the TUI's screen and view without a terminal. What no gate sees is the real terminal
loop (raw mode, the alternate screen, the restore) and anything only a person judges.
That is what running is for, and its result is evidence in the pull request, never a
substitute for a test.

## What an agent runs on its own

Nothing here opens a window, takes focus, raises a prompt, or takes over a terminal
(`AGENTS.md` › "Never taking over the developer's Mac"). This is the default, and
usually enough.

```bash
just test-platform   # platform adapters and the bunshin binary against the real OS
just logs            # the newest log file's last 50 lines, then exit
```

- **The available noninteractive commands** are `--help` and `--version`:

  ```bash
  cargo run --locked -p bunshin -- --version
  cargo run --locked -p bunshin -- --help
  ```

  These return before file logging starts. Future data subcommands are run against
  an isolated `HOME` when any log or application write is possible. Build first with
  the normal environment, then launch the built binary: changing `HOME` for cargo
  also changes where rustup looks for its toolchain. On Linux unset both
  `XDG_DATA_HOME` and `XDG_STATE_HOME` to keep paths inside the scratch home.
- **`just logs`** prints the tail of the newest `bunshin.YYYY-MM-DD.log` (dated in UTC)
  in the log directory `bunshin_platform::log_dir` picks: `~/Library/Logs/<bundle id>/`
  on macOS, `$XDG_STATE_HOME/bunshin/logs` (default `~/.local/state/bunshin/logs`) on
  Linux. A log line is often the cheapest observable for a wiring change: add the
  `tracing` event in the binary, run, then read it.

## When only a real terminal can show it: ask once

Some changes are only visible in an interactive terminal: how the screen looks, a key
that does nothing, a resize, the terminal left broken after an error or a crash. An
agent never runs `bunshin tui`, not even to see it refuse: in an agent shell backed by a
terminal it would take that terminal over and block. Its refusal without a terminal is
already tested (`tui_without_a_terminal_fails_with_exit_code_1_and_touches_nothing` in
`crates/bunshin/tests/cli.rs`). The agent asks the human, once, in one message, before
iterating:

- the exact commands: `cargo build --locked -p bunshin` first, then, against a scratch
  `HOME` so the run touches none of their data, `HOME="$scratch" target/debug/bunshin tui`
  (not `HOME="$scratch" cargo run …`, which makes rustup fetch a toolchain into it);
- the keys to press, in order, and what to look for after each, including leaving
  with `q` and checking that the shell prompt and the cursor came back;
- any privacy grant or System Settings step, all at once (`integrating-system-apis`);
- what to send back: what they saw, and a screenshot when it matters.

Then read what the run recorded (`tui opened`, each action, `tui closed`) yourself,
with `tail` on the log under the scratch directory (`$scratch/Library/Logs/<bundle id>/`
on macOS, `$scratch/.local/state/bunshin/logs/` on Linux); `just logs` reads the real
`HOME`'s directory instead. `just logs-follow` never ends and is the human's.

## Putting the tool in a known state

Do not add a flag or an environment switch only to look at a state. A state you only
need to see is one a test can build directly: a core test or a `TestBackend` test constructs
the state directly (`ShellScreen::default()` or a `day::Day` fixture), and a `cli.rs`
test writes any legacy-file fixture into its temporary `HOME`. For a human's run, write that file into a
scratch `HOME` the same way. A start state genuinely needed by hand as well as by tests
is read once in the composition root and handed to core as a value, so a core test
still reaches it; core never reads the environment (`designing-core-logic`).

## The evidence a pull request carries

No gate runs any of this, so the pull request is where it lands. State the exact
command, not a paraphrase, and paste:

- the command and its stdout, stderr, and exit code, for a subcommand's behavior;
- a `just logs` excerpt, with the command that produced it, for behavior whose only
  observable is a log line;
- what the human saw in `bunshin tui`, with a screenshot for anything a person looks at;
- `just test-local` output, run by a human, for a change to an adapter with an
  `#[ignore = "local machine: …"]` test (`AGENTS.md` › "Review Checklist").

Redact before pasting: a personal name or a home directory path (`/Users/<name>/…`,
`/home/<name>/…`, a scratch directory under it), and say that you did. A pull request
here, or in an app cut from this template, may be public. Leave nothing behind: remove
any scratch `HOME`, and check `git status --porcelain` shows only the change.
