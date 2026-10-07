# UX guidelines

| | |
|---|---|
| Product | Bunshin — a three-character bulletin board in a terminal pane |
| Platforms | macOS 27+ terminal emulators (Terminal.app, iTerm2, Ghostty); the same TUI builds on Linux, where the model is unavailable |
| App type | Conversational board UI |
| Related | Requirements: [`requirements.md`](../product/requirements.md) · UX flows: [`ux-flows.md`](../product/ux-flows.md) (screens, layouts, flows, key table) · Design direction: [`design-direction.md`](design-direction.md) (the color roles and their measured contrast) |

## Principles

- **One shared board.** Characters and the owner post to the same list; there are no
  threads. New posts appear at the bottom.
- **Owner posts are immediate.** The owner's post appears before a model call; typing
  never waits for a character's reply.
- **A still screen that says it in words.** No animation, and no state shown by color or
  symbol alone. The header names the current status.

## Navigation

One screen with a board and an input line; there are no overlays. Tab and Shift+Tab
move focus between the board and input. The screen layout and key table are in
ux-flows.md.

## Platform conventions

- **Terminal:** runs in the alternate screen and restores the terminal on every way out
  (the template's `building-tuis`). It binds no Cmd-based shortcut.
- **No mouse capture.** The app never turns on mouse reporting, so the terminal's own
  text selection and copy keep working.
- **Japanese input:** the terminal composes Japanese input. While the input has focus,
  the cursor sits at the insertion point so the IME's candidate window opens there. In
  board focus, a full-width character is read as its ASCII form; while the IME composes
  kana it holds keys until they commit.
- **Character widths:** width follows Unicode East Asian Width — Japanese is two
  columns; ambiguous-width characters (box drawing, arrows, ×, ⇔) count as one. A
  terminal set to treat ambiguous characters as wide may misalign borders. No emoji in
  the chrome; model text wraps by measured width.
- **Linux:** the same screen; the model is reported unavailable (requirements §4).

## States

Copy for the app's own sentences is Japanese and lives in the binary's `wording.rs`.

| State | Trigger | Shows | Primary action |
|---|---|---|---|
| Open | `bunshin tui` starts | Empty board; the first character attempt begins | Post or wait |
| Idle | No model call is writing | 待機中 in the header | Post |
| Writing | A call lasts past `thinking_after` | 書き込み中 in the header | Post or wait |
| Character post | A non-empty model body succeeds | New row at the bottom | Read or scroll |
| Owner post | Enter on non-blank input | Immediate row labelled あなた; character response posts follow | Continue |
| Call failed | Unavailable model, timeout, malformed, failed, or empty result | Last failure in the header; no new post | Post or wait for the next interval |
| Scrolled | The owner is reading older posts | New posts do not move the viewport; the new-post divider appears | End returns to latest |
| Model unavailable | `fm` cannot write a character post | Failure status in the header; the board and input remain usable | Post or quit |
| Quit | The quit key is pressed | The screen exits immediately; the in-memory board is lost | — |

## Feedback and loading

| Rule | Value |
|---|---|
| Writing status | Shown in the header after `thinking_after`; no spinner or temporary post |
| Long calls | One model call at a time, with the existing 30 s timeout |
| Replies | One structured post arrives as a whole; no streaming |
| Failure | The board stays unchanged; the failure remains in the header until a successful post |
| Interval | The next character attempt starts 30 seconds after the previous attempt finishes |
| Owner input | The owner's post appears immediately, including while a character call is in flight |
| New posts | When scrolled up, new rows do not move the viewport; End returns to latest |
| Bell | None |
| Error placement | A model-call failure appears in the header; no failed post is added |
| Quit | Immediate; there is no save confirmation |

## Forms and validation

- The single-line input accepts up to 400 characters.
- Enter on blank or whitespace-only input does nothing.
- A non-blank owner post appears immediately; character generation never delays input.

## Motion

- None: no animation, spinner, blinking, or transition.
- The screen redraws on an event — a key, a model result, a resize, or a post attempt — and
  when the clock changes.
- Reduced motion: nothing to reduce. The terminal's own cursor blink is the terminal's
  setting.

## Language and copy

- UI language: Japanese only — labels, the help supplied by the app, status, and
  errors. No internationalization layer.
- Times in the header and post rows use 24-hour HH:MM.
- The app's own sentences use です・ます, plain and without exclamation marks or emoji.
  Character posts use the fixed character voices; the app does not rewrite them.
- Speaker labels are ハル, シズク, ゲン, and あなた. Errors say what happened and the next
  step in Japanese.

| Use | Don't use | For |
|---|---|---|
| ボード | タスク一覧 | the shared post list |
| 投稿 | メッセージ | a row written by a character or the owner |
| 登場人物 | ペルソナ | one of the three fixed characters |
| あなた | ユーザー | the owner's speaker label |

## Accessibility targets

The web criteria are carried over where a terminal has an equivalent; pointer and
reflow criteria have none.

| Target | This product |
|---|---|
| Level | WCAG 2.2 AA where a terminal applies it: everything is text, every function by keyboard, nothing by color alone, focus visible |
| Focus | The focused board or input has a visible focus indication; the input cursor stays at its insertion point |
| Minimum target size | Not applicable: no pointer |
| Keyboard | Every action is named in the UX flow key table |
| Announcements | Header status and post text carry meaning; no bell |
| Color use | Never the only carrier; post text and header words carry every state |
| Text scaling | The terminal's font size; the layout holds down to 60 × 18 |
| Contrast | Text in the default color, red, blue, or magenta only, green only on the focused border, no faint text (`design-direction.md`) |
| Motion | None |

The TUI's view is tested with ratatui's `TestBackend`. The owner checks behavior that only
a real terminal can show: VoiceOver, a monochrome profile, and Japanese input. An agent
never runs `bunshin tui`.

## App-type rules

- **Show what it is doing:** the header says idle, writing, or the last failure.
- **Keep the owner in control:** a post is added only by a character turn or an explicit
  owner submission; the model does not choose the speaker or act outside the board.
- **Keep the conversation readable:** one list, newest at the bottom, with no threads,
  overlays, bell, or notification.

## Non-goals

| Not doing | Why | Covered instead by |
|---|---|---|
| Mouse support | it would take the terminal's text selection away | the key table |
| Animation, spinners | the terminal stays still; words say the state | header words |
| Streaming replies | short replies, structured output | complete structured posts |
| RGB or 256-color values | the owner's theme decides how the 16 named colors look | `design-direction.md` |
| A second language | one owner, who reads Japanese | — |

## Open items

- None. The Japanese user-facing wording, an exception to AGENTS.md's English-only rule
  for code and documents, is recorded in the `deciding-architecture` skill (2026-10-02)
  and stated in `AGENTS.md` › "Important Reminders".

## Decision log

Entries dated 2026-10-02 describe the former secretary interface; the current rules above
describe the board prototype.

| Date | Decided | Rejected options | Why |
|---|---|---|---|
| 2026-10-02 | Thinking shown as still text after 300 ms; 「まだ考えています（Esc で中止）」 at 10 s; Esc cancels the owner's call | a spinner; no cancel | still text reads to a screen reader and keeps the screen calm; a slow call must be stoppable |
| 2026-10-02 | One bell for every unprompted message, catch-ups included | questions only; never | a deadline note matters as much as a question when the terminal is behind other windows |
| 2026-10-02 | Japanese only, です・ます for the app's own sentences | short plain style; English | the conversation is Japanese; a neutral polite voice keeps the app apart from the secretary |
| 2026-10-02 | Replies shown whole | streaming | replies are ≤ 200 chars in 1–4 s and arrive in the same structured output as the changes |
| 2026-10-02 | Set from the inputs, not asked: no mouse capture; full-width keys read as ASCII; ambiguous width counted as one; a 新着 divider and a 新着 count while scrolled up; quitting while saves fail asks once; an unreadable day file stops the TUI and is never overwritten; the speaker label システム for the app's notices | — | each follows from a requirement or a template rule; listed for the owner's review |
| 2026-10-02 | Color: the terminal's 16 named colors, text only in red, blue, and magenta, green only on the focused border; no faint text (`design-direction.md`) | monochrome with modifiers; a fixed RGB palette | the secretary and the errors stand out, and the owner's theme still decides how they look |
| 2026-10-07 | One board, no bell; the owner's post appears immediately and characters respond with posts | a task pane, threads, or notifications | the prototype tests one shared conversation loop |
