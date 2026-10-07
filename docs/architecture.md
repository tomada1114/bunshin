# Architecture

This page describes the layers every app cut from the template starts with, how they
talk, and which boundaries are contract. Bunshin's current board design and its recorded
decisions live in the [deciding-architecture skill](../.agents/skills/deciding-architecture/SKILL.md).
The reasoning behind the template layers is in the README's
[Design Philosophy](../README.md#design-philosophy).

## Layers

```text
┌───────────────────────────────────────────────────────────────────────────┐
│ crates/bunshin  the binary and composition root                           │
│ Runs the terminal interface and wires core to the real adapters.          │
└───────────────────────────────────────────────────────────────────────────┘
      │ constructs adapters and hands them to core
      ▼
┌───────────────────────────────────────────────────────────────────────────┐
│ crates/bunshin-platform  adapters for the OS and external processes       │
└───────────────────────────────────────────────────────────────────────────┘
      │ implement core's ports
      ▼
┌───────────────────────────────────────────────────────────────────────────┐
│ crates/bunshin-core  domain rules, state, ports, and screen models         │
│ No OS APIs, terminal APIs, or direct I/O. Builds and is tested on Linux.   │
└───────────────────────────────────────────────────────────────────────────┘
  crates/bunshin-test-support  fakes and port contract functions
                               (dev-dependencies only; never ships)
```

Dependencies point toward core: `bunshin-platform` → core; the binary → platform and
core. Core depends on neither. Everything runs in one process; there is no IPC layer.
For Bunshin's board state, turn selection, model worker, and screen, see the current
design document linked above.

## How the boundaries are enforced

- `bunshin-core/Cargo.toml` names no OS or platform crate.
- `just check-harness` checks the core dependency closure against the forbidden-crate
  list in `AGENTS.md` and rejects non-dev dependencies on test support.
- `deny.toml` allows `bunshin-platform` as a direct dependency only from the binary.
- Core's clippy configuration bans direct I/O, OS access, clock reads, process
  execution, and unscoped threads. `just lint` checks these bans and fails when a
  configured path cannot be resolved.
- Core enums are matched exhaustively so a new failure state must be handled explicitly.

The lists and enforced values are maintained together; `just check-harness` verifies
the harness claims.

## Ports and adapters

Core declares synchronous `Send + Sync` traits for anything it needs from outside the
process. The platform crate implements those traits, the test-support crate provides
fakes, and the binary constructs the real adapters. Core remains deterministic: it
receives time, model responses, and other external facts through arguments or ports.

Each port has a typed error owned by core. The binary translates those errors into
user-facing wording and an exit code; adapters translate OS failures at the boundary.
Core does not print, log, or build user-facing sentences.

## The binary

The `bunshin` binary has one subcommand, `bunshin tui`; `--help` and `--version`
remain available. The command-line contract is tested against the built binary with a
temporary `HOME`.

- **Streams.** The TUI owns the terminal and writes no data to stdout or stderr while
  drawing. Startup diagnostics go to stderr; logs go to the log file while the screen
  is active.
- **Exit codes.** 0 on normal quit and for `--help` or `--version`, 1 on a runtime
  error, and 2 on a clap usage error.
- **Startup.** The binary confirms stdin and stdout are terminals, constructs the
  adapters and board, then starts the terminal loop.
- **Screen.** Core owns the board state and key actions; the binary owns the TUI loop,
  model worker, and ratatui view. No check runs the real loop: a human runs
  `bunshin tui`.

## Logging

The binary and `bunshin-platform` log through `tracing`; core has no tracing
dependency and logs nothing. Only the binary installs the subscriber. Log files are
named `bunshin.YYYY-MM-DD.log` under
`~/Library/Logs/io.github.tomada1114.bunshin/` on macOS and
`$XDG_STATE_HOME/bunshin/logs/` on Linux (default
`~/.local/state/bunshin/logs/`). The newest 14 are kept. Logging is best effort and
carries no user data. A line on the terminal while the TUI owns it would corrupt the
frame. `just logs` prints the newest file's last lines and exits.

## Where new code goes

| You are adding… | It goes in… | Tested by… |
|---|---|---|
| A domain rule, state change, or view model | `crates/bunshin-core` | core tests and coverage-gated `just test-core` |
| OS or filesystem access | an adapter in `crates/bunshin-platform`, behind a core port | shared contract tests against fakes and real adapters |
| A command-line argument or error wording | `crates/bunshin` (`src/wording.rs`) | built-binary tests and wording tests |
| A screen state or key action | core, beside its screen model | core tests with keys and actions as values |
| Screen drawing | `crates/bunshin/src/tui/` | ratatui `TestBackend` tests |
| Terminal lifecycle or wiring | `crates/bunshin/src/tui/` and `src/main.rs` | no automated check runs the loop; a human runs `bunshin tui` |

## What is contract and what is private

The public core API, bundle identifier, command line, and log locations are contracts.
The command line consists of the single `tui` subcommand, `--help`, `--version`,
and exit codes 0 (success), 1 (runtime error), and 2 (usage error). The board exists
only in memory; no user-data file format is defined. Earlier application data is not
read or deleted.

Log files are named `bunshin.YYYY-MM-DD.log` under the platform log directory described
above. Their wording and internal format are private. Also private are crate-private
items, adapter internals, screen layout and styling, file and module layout, and test
helpers. Change the public API only with its callers in the same pull request, and treat
visible command-line changes as breaking.

The compiler checks core API callers, and the binary tests check command-line behavior.
Review checks that the documented contracts remain accurate.
