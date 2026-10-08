# Getting Started

## Prerequisites

- A Mac with Apple Silicon, or Linux.
- On a Mac, the Xcode Command Line Tools (`xcode-select --install`); the full Xcode app
  is not needed. On Linux, a C toolchain for the linker (`build-essential` on Debian and
  Ubuntu).
- [rustup](https://rustup.rs/), [mise](https://mise.jdx.dev/), and
  [Just](https://just.systems/man/en/) (`brew install mise just`).

[CONTRIBUTING.md](../CONTRIBUTING.md) lists what each one provides.

## Setup

```bash
mise trust     # approve this repository's mise.toml (asked once per clone)
just install
```

Without trust, mise prompts, skips the config, or fails when it cannot prompt, so a fresh
clone runs `mise trust` first (<https://mise.jdx.dev/cli/trust.html>, checked
2026-09-28).
`just install` then:

1. on a Mac, checks for the Xcode Command Line Tools and, if they are missing, stops
   with the command to run (it never starts an installer);
2. runs `mise install` for the pinned tools;
3. installs lefthook's pre-commit hook, and fails if it is not in place.

The first `cargo` command installs the Rust toolchain `rust-toolchain.toml` pins.

## Everyday commands

```bash
just check       # the full local gate, in CI's order; takes over no terminal
just test        # core and xtask, with their coverage floors
just test-fast board        # one core test or a group of them
just lint        # rustfmt, clippy -D warnings
just fmt         # format everything
```

## Seeing the app

`just check` never runs the tool interactively. To read what it logged:

```bash
just logs        # the newest log file's last lines
```

The screen is a shared board. Opening `bunshin tui` starts a character turn. Posts stay
in memory and disappear when the process exits. Type a non-blank post and press Enter;
it appears immediately, one character replies as soon as the model worker is free, and a
different character posts a second response after the next 30-second interval. Tab moves
between the input and board; PgUp and PgDn scroll, End returns to the newest posts, and
`q` quits with the board focused. Ctrl+C quits anywhere. Logs go to
`~/Library/Logs/io.github.tomada1114.bunshin/` on macOS and
`$XDG_STATE_HOME/bunshin/logs/` on Linux (default `~/.local/state/bunshin/logs/`).

```bash
cargo run --locked -p bunshin -- --help
cargo run --locked -p bunshin -- tui   # a human's terminal: Ctrl+C quits anywhere
```

`tui` takes over the terminal you run it from until you quit, and restores it on the
way out. It is yours to run: no check and no agent starts it.

To run `bunshin` from any directory, `just install-cli` installs it into `~/.cargo/bin`
(`cargo install --locked --path crates/bunshin`). It writes outside the checkout, so it is
a human's recipe that no check and no agent runs unasked. There is no other
distribution: no release artifacts, no installer.

## Permissions (TCC)

The sample asks for no privacy permission. When an app cut from the template does on
macOS — Accessibility, Full Disk Access, and the like — `just test-local` runs the
`#[ignore]`d tests that need a logged-in session, a TCC grant, or the Keychain. You
start it; nothing else does. macOS grants such a permission to the program that asks,
so how a tool installed with `cargo install` keeps its grant across rebuilds is a
decision for that app to record.

## The domain foundation

Core's `board` module owns post state and turn selection. Its `screen` module owns the
board's key state and text input; the binary drives the model worker and draws the view,
which is covered with ratatui's `TestBackend`. The unused `day` module still contains
the former task transitions and versioned file DTOs. `Clock`, `SystemClock`,
`FixedClock`, and `clock_contract` illustrate the same port, adapter, fake, and contract
split used for I/O features. Real terminal lifecycle evidence comes from the owner's
run.
