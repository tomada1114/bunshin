# Test patterns

Each example illustrates a rule independently of the feature using it. Keep the rule
when replacing its illustration; use the cheapest seam that can fail for the behavior.

## State transitions and error paths

Construct a valid `day::Day`, apply a task change, and assert the resulting task view
against literal values. For a rejected title, match `DayError` rather than its display
text, and assert the day is unchanged. The fixture and the expected answer must not
call the implementation's private helpers.

## Injected time

```rust
use bunshin_core::{Clock, UnixMillis};
use bunshin_test_support::FixedClock;
let clock = FixedClock::at(UnixMillis(12), jiff::tz::Offset::UTC).unwrap();
assert_eq!(clock.now().instant, UnixMillis(12));
```

Read time through `Clock`, never sleep. `FixedClock` also permits a fixed UTC offset so
civil-day tests do not depend on the host zone. Expected logical dates are literal
calendar values, including the instant before and at the boundary.

## Tables and boundaries

Pair each input with a literal expected answer; do not recreate the implementation's
formula. Title limits need empty, at-limit, and over-limit cases. Task budgets include
delete and undo, since neither replenishes the creation budget.

## One shared contract

```rust
use bunshin_test_support::{FixedClock, clock_contract};
clock_contract(|| Box::new(FixedClock::default()));
```

The platform suite runs the same `clock_contract` against `SystemClock`. A port's fake
and real adapter must satisfy every promise in its documentation.

## Screen sequences and drawing

Feed `ShellKey` values through `ShellAction::for_key`, then update `ShellScreen` and
assert the finished state. Unbound keys yield no action. Draw the screen into a
`Terminal<TestBackend>` and assert literal lines (and styles when meaningful), never
run the real terminal loop in a test. Include a small frame to verify clipping.

## Built binary and wording

Run `CARGO_BIN_EXE_bunshin` with an isolated `HOME`, unsetting both XDG variables.
Assert stdout, stderr, and the exit code. Nonterminal `tui` refuses with one stderr
line and creates no files. Usage errors exit 2; runtime errors exit 1. Each future
error variant gets one wording assertion, separate from core's typed-error test.
