# Test patterns

Each example illustrates a rule independently of the feature using it. Keep the rule
when replacing its illustration; use the cheapest seam that can fail for the behavior.

## State transitions and error paths

Construct a `board::Board`, add an owner post, finish a selected turn, and assert the
resulting post against literal values. For a malformed model answer, assert the typed
failure and that the board has no new post. The fixture and expected answer must not
call the implementation's private helpers.

## Injected time

```rust
use bunshin_core::{Clock, UnixMillis};
use bunshin_test_support::FixedClock;
let clock = FixedClock::at(UnixMillis(12), jiff::tz::Offset::UTC).unwrap();
assert_eq!(clock.now().instant, UnixMillis(12));
```

Read time through `Clock`, never sleep. `FixedClock` also permits a fixed UTC offset so
display-time tests do not depend on the host zone. Expected times are literal values.

## Tables and boundaries

Pair each input with a literal expected answer; do not recreate the implementation's
formula. Owner input limits need empty, at-limit, and over-limit cases. Board history
limits include repeated posts, since the oldest post is dropped at capacity.

## One shared contract

```rust
use bunshin_test_support::{FixedClock, clock_contract};
clock_contract(|| Box::new(FixedClock::default()));
```

The platform suite runs the same `clock_contract` against `SystemClock`. A port's fake
and real adapter must satisfy every promise in its documentation.

## Screen sequences and drawing

Feed `ScreenKey` values to `screen::BoardScreen::update` and assert its board or finished
state. Unbound keys have no effect. Draw the screen into a `Terminal<TestBackend>` and
assert literal lines (and styles when meaningful), never run the real terminal loop in a
test. Include a small frame to verify clipping.

## Built binary and wording

Run `CARGO_BIN_EXE_bunshin` with an isolated `HOME`, unsetting both XDG variables.
Assert stdout, stderr, and the exit code. Nonterminal `tui` refuses with one stderr
line and creates no files. Usage errors exit 2; runtime errors exit 1. Each future
error variant gets one wording assertion, separate from core's typed-error test.
