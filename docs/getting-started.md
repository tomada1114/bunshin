# Getting Started

## Prerequisites

- A Mac with Apple silicon, or Linux.
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

The first `cargo` command installs the Rust toolchain pinned in `rust-toolchain.toml`.

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

Run the board in a terminal with:

```bash
cargo run --locked -p bunshin -- tui
```

The screen has a header, a scrolling board, and an input line. The three fixed characters
post to the shared board; type a message and press Enter to join them. PgUp and PgDn
scroll, End returns to the latest posts, and Ctrl+C quits. The board exists only in
memory and is discarded on quit. The app does not read or remove earlier application
data. No check or agent runs the real terminal loop; you can try it yourself.

The only command-line subcommand is `tui`; `--help` and `--version` are also available.
To run `bunshin` from any directory, `just install-cli` installs it into
`~/.cargo/bin` (`cargo install --locked --path crates/bunshin`). It writes outside the
checkout, so it is a human's recipe that no check and no agent runs unasked. There is no
other distribution: no release artifacts or installer.

Logs go to `~/Library/Logs/io.github.tomada1114.bunshin/` on macOS and
`$XDG_STATE_HOME/bunshin/logs/` on Linux (default
`~/.local/state/bunshin/logs/`).

## Permissions (TCC)

The app asks for no privacy permission. If an app cut from the template later needs a
macOS permission such as Accessibility or Full Disk Access, `just test-local` runs the
tests marked for a logged-in session, a TCC grant, or the Keychain. A human
starts that recipe. macOS grants a permission to the program that asks, so how a tool
installed with `cargo install` keeps its grant across rebuilds is a decision that app
must record.

## The domain foundation

Core owns the board's bounded in-memory state, character-turn selection, post creation,
and screen actions. The `Clock` and `LanguageModel` ports keep time and model access
outside the domain; platform adapters call the real system and model, while tests use
fakes. The binary owns the single model worker, terminal loop, view, and user-facing
wording. The architecture skill records the current decisions; the product requirements
and UX flows describe visible behavior.
