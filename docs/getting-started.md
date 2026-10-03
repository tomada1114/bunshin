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
just test-fast logical_date   # one core test or a group of them
just lint        # rustfmt, clippy -D warnings
just fmt         # format everything
```

## Seeing the app

`just check` never runs the tool interactively. To read what it logged:

```bash
just logs        # the newest log file's last lines
```

The shell opens an empty day and offers `q` and Ctrl+C to quit. The clock adapter
and deterministic day model remain available for the next product features; the shell
reads no day file. Data-directory conventions remain unchanged, and old sample data
is left untouched. Logs go to `~/Library/Logs/io.github.tomada1114.bunshin/` on macOS
and `$XDG_STATE_HOME/bunshin/logs/` on Linux (default `~/.local/state/bunshin/logs/`).

```bash
cargo run --locked -p bunshin -- --help
cargo run --locked -p bunshin -- tui   # a human's terminal: q or Ctrl+C quits
```

`tui` takes over the terminal you run it from until you quit, and restores it on the
way out. It is yours to run: no check and no agent starts it.

To run `bunshin` from any directory, `just install-cli` installs it into `~/.cargo/bin`
(`cargo install --locked --path crates/bunshin`). It writes outside the checkout, so it is
a human's recipe that no check and no agent runs unasked. There is no other
distribution: no release artifacts, no installer.

Read today's saved task list with `cargo run -p bunshin -- today`, or add `--json`
for its stable versioned task view. The command works while a screen holds the writer
lock. It does not create a missing day, change files, or invoke the model.

## Permissions (TCC)

The sample asks for no privacy permission. When an app cut from the template does on
macOS — Accessibility, Full Disk Access, and the like — `just test-local` runs the
`#[ignore]`d tests that need a logged-in session, a TCC grant, or the Keychain. You
start it; nothing else does. macOS grants such a permission to the program that asks,
so how a tool installed with `cargo install` keeps its grant across rebuilds is a
decision for that app to record.

## The domain foundation

Core's `day` module holds task transitions, undo, and versioned file DTOs. The shared
`Tuning` holds `DayTuning` and the logical-day boundary. `Clock`, `SystemClock`,
`FixedClock`, and `clock_contract` illustrate the same port, adapter, fake, and
contract split used for future I/O features. Core's `shell` module holds the empty
screen's state and one key table; the binary draws it with ratatui's `TestBackend`
covering the view. Real terminal lifecycle evidence comes from the owner's run.
