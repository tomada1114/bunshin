# Bunshin's design as it stands

Bunshin is a small, single-user bulletin board in a terminal pane. Three fixed characters
post to one shared board, and the owner can join them. The product values are in
[requirements.md](../../../../docs/product/requirements.md); decisions and rejected
options are in [decision-log.md](decision-log.md).

## Principles

Each one rules something out.

1. **Core chooses; the model writes.** Core selects the speaker, post kind, and topic.
   The model returns only one character's post body.
2. **Core stays synchronous and deterministic.** Time and the model arrive through ports.
   A fixed clock, seed, and scripted model reproduce a conversation in a test.
3. **One model worker.** Calls never overlap. A slow call does not stack another attempt.
4. **The board is temporary.** Posts live only in memory, up to 200†, and disappear on
   quit. The app does not read or remove earlier user data.
5. **The board stays usable without the model.** A failed character call adds no post;
   the owner can still post and quit.

## Crates and ports

The four workspace crates stay; no crate is added.

| Crate | Holds |
|---|---|
| `bunshin-core` | board state, speaker and turn selection, topic values, post construction, tuning, and screen actions |
| `bunshin-platform` | clock and model adapters, terminal-independent OS integration, and log paths |
| `bunshin-test-support` | clock and model fakes and their contract functions |
| `bunshin` | the command line, TUI loop, one model worker, view, and user-facing wording |

Core declares two synchronous `Send + Sync` ports:

| Port | Real adapter | Fake | What it provides |
|---|---|---|---|
| `Clock` | `SystemClock` | `FixedClock` | an instant for spacing and random seeding, plus local time for post labels |
| `LanguageModel` | `FmLanguageModel` on macOS; `UnavailableLanguageModel` on Linux | `ScriptedLanguageModel` | availability and one structured response or a typed `ModelError` |

No port performs asynchronous work. The binary supplies adapters to core and owns the
single worker thread.

## Dependencies

The board needs no new crate. Its seeded xorshift generator is a few lines in core; the
random choice is reproducible without a random-number dependency. Existing JSON support
in core unwraps a body the model still writes as a JSON object.

## Data

The board is a bounded, in-memory list of posts and is not persisted. Earlier
application data remains untouched and unread; daily logs remain the only files the
board app writes.

## Model call

One character post uses one `fm respond` call through `LanguageModel`. Core chooses the
speaker and task first; the model receives that speaker's persona, the casual-board
rules, the latest 8† posts (none for a New topic), and the task. The owner is named
ユーザー in prompts so the model never confuses them with the "あなた" it is told to
play. The call asks for plain text, with no schema; `LanguageModel` returns the raw
UTF-8 text, and core cleans it (unwraps a JSON body, cuts leftover JSON, code fences,
and control markers, removes a name prefix and wrapping quotes). An empty body fails,
and displayed text is cut to 120† characters. The instruction asks for at most 60†
characters. Calls use the existing 30-second timeout.

## Main flows

- **Open.** Create an empty board and seed the generator once from the clock's instant.
  Start the first character attempt immediately.
- **Character post.** Core selects a speaker and kind, then requests one body. On
  success, append the post. Start the next attempt 30 seconds after the previous attempt
  finishes, whether it succeeded or failed.
- **Owner post.** Add the owner's post immediately. A random character writes a reply as soon as the worker is free; a different
  character writes a second response post at the next interval. If the owner posts again
  first, both pending response posts target the newest owner post.
- **Failure.** Add nothing and show a short failure in the header until a successful
  post. The next attempt waits for its regular interval.
- **Quit.** Exit immediately. There is no save confirmation because the board is
  temporary.

## Language

The screen and the owner's posts use Japanese. Each character has a fixed name, role,
voice, and persona text. The user-facing sentences remain in `wording.rs`.

## Color lock

The role table in `docs/design/design-direction.md` remains the color lock: the terminal's
16 named colors, text only in red, blue, and magenta, green only on the focused border,
reversed selection, and no faint text.

## Platforms and distribution

The target remains macOS 27 or later on Apple silicon. Linux builds and reports the model
unavailable. No TCC permission or distribution mechanism is added; the binary is installed
from its checkout.

## Quality targets

| Target | Value | How it is checked |
|---|---|---|
| Post interval | 30 seconds after attempt completion | core tests with a fixed clock |
| Concurrent model calls | at most one | worker and screen tests |
| Failed model call | no post; failure remains in the header | core and view tests |
| Owner post | visible immediately; two response posts target the newest owner post | screen tests |
| Board size | at most 200† posts | core tests |
| Core coverage | lines 80, functions 80 | `just test-core` |

## Open

None for this prototype's accepted architecture.
