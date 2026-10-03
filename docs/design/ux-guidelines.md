# UX guidelines

| | |
|---|---|
| Product | Bunshin — a proactive secretary in a terminal pane |
| Platforms | macOS 27+ terminal emulators (Terminal.app, iTerm2, Ghostty); the same TUI builds on Linux, where the model is unavailable |
| App type | AI assistant / agent UI, with a productivity (tasks) core |
| Related | Requirements: [`requirements.md`](../product/requirements.md) · UX flows: [`ux-flows.md`](../product/ux-flows.md) (screens, layouts, flows, key table) · Design direction: [`design-direction.md`](design-direction.md) (the color roles and their measured contrast) |

## Principles

- **Nothing changes silently** — every change to the list, from words, keys, or a
  carry-over, is a change line in the chat and undoable. Decides: no change is applied
  without a visible line, and no confirmation dialogs are needed. Gives up: a busier chat.
- **Speak little, drop nothing** — unprompted messages are spaced by the core's guards,
  but whatever a guard holds (the mute, active hours, typing) comes back as one message
  later. Decides: held triggers are summarized, never discarded within the day. Gives
  up: a catch-up can arrive as a lump after a long mute.
- **A still screen that says it in words** — no animation, and no state shown by color
  or symbol alone: every state is a word in the header or a text marker. Decides: the
  thinking state is the text 「考え中…」, not a spinner. Gives up: the screen feels less
  alive.

## Navigation

One screen with overlays; ux-flows.md §2 and §4 own the layout, the focus model, and
the key table. Esc, or the key that opened it, closes any overlay and returns focus to
where it was.

## Platform conventions

- **Terminal:** runs in the alternate screen and restores the terminal on every way out
  (the template's `building-tuis`). Uses only keys a terminal passes through: no
  Cmd-based shortcut is ever bound. In raw mode Ctrl+Z is an ordinary key, bound to undo,
  not to job control.
- **No mouse capture.** The app never turns on mouse reporting, so the terminal's own
  text selection and copy keep working on chat text. Why: copying a line out of the chat
  matters more than clicking, which the keyboard already covers.
- **Japanese input:** the terminal composes Japanese input. While the input has focus,
  the cursor sits at the insertion point so the IME's candidate window opens there. In
  the task pane and the overlays, a full-width character is read as its ASCII form
  (NFKC: `ａ` → `a`, the ideographic space → Space), so the keys work with the IME in
  full-width alphanumeric mode; while the IME is composing kana it holds keys until it
  commits, and T3 says to switch to 英数 for keys.
- **Character widths:** width follows Unicode East Asian Width — Japanese is two
  columns; ambiguous-width characters (box drawing, arrows, ×, ⇔) count as one, which
  is the default in Terminal.app, iTerm2, and Ghostty. A terminal set to treat ambiguous
  characters as wide misaligns the borders; the README says so, and the app does not
  work around it. No emoji in the chrome (their width differs between fonts); text the
  model writes may contain them and wraps by its measured width.
- **Linux:** the same screen; the model is reported unavailable (requirements §4).

## States

Copy is in Japanese, です・ます (see Language and copy); wording lives in the binary's
`wording.rs` with the rest of the app's sentences.

| State | Trigger | Shows | Primary action | Copy pattern |
|---|---|---|---|---|
| First run | no day on record | T1-d: the 指示文 notice once, then Bunshin asks for the plan | write the plan | 「指示文は既定のものを使っています。自分の言葉で書くには、端末で bunshin instructions edit を実行してください。」 |
| Empty day | the day start settled, no tasks | the task pane's one-line hint | write the plan, or `a` | 「（まだありません ─ 予定を入力欄に書くか、Tab で移って a で追加）」 |
| Thinking | a call for the owner's message passes 300 ms | 「考え中…」 in the header; a 「…」 row in the chat | wait, or Esc | — |
| Long wait | the same call passes 10 s | the chat row becomes 「まだ考えています（Esc で中止）」 | Esc | — |
| Cancelled | Esc during the call | the message stays in the chat with 「（中止）」 after it; nothing changes; its text goes back to the input if the input is empty | resend | 「中止しました。変更はありません。」 |
| Checking in | a trigger's call is running | 「見回り中…」 in the header only; no chat row, no cancel | — | — |
| Call failed | timeout (30 s†), a guardrail refusal, or output that fails the format | one エラー row | resend | 「うまく読み取れませんでした。もう一度送ってください。」 |
| Model unavailable | `fm` missing, terms not accepted, model not ready, Linux | 「モデル: 使えません」 in the header; one システム row naming the cause and the owner's step | the step named | ux-flows.md, "When the model is unavailable" |
| Model back | the 10-min† recheck succeeds | the header returns to 「待機中」; one システム row | — | 「モデルが使えるようになりました。」 |
| Outside active hours | 22:00†–08:00† | 「時間外（08:00 から）」 in the header | — | — |
| Muted | `m`, or a mute by chat | 「ミュート中 〜HH:MM」 in the header; a change line | `m` to unmute | 「ミュート 〜16:31（m で解除）」 |
| Held while typing | an unprompted message is due while the input has text | 「保留 N」 in the header | send or clear | — |
| Save failing | a write of the day fails | 「保存できません」 in the header until a write succeeds; one エラー row with the cause; state kept in memory and written again on every change | free disk space, fix permissions | 「保存できませんでした（<cause>）。変更は画面に残っていて、次の変更のときにもう一度保存します。」 |
| Durability unconfirmed | directory sync fails after the complete new day becomes visible | 「保存済み・耐久性未確認」 in the header, a distinct error row and retry on the next change | inspect storage; next change retries | 「新しいデータは保存済みですが、耐久性は未確認です。次の変更のときにもう一度保存します。」 |
| Quit while durability is unconfirmed | `q` or Ctrl+C with the durability flag | a distinct confirmation on the help line | `y` quits, any other key stays | 「保存済み・耐久性未確認です。終了しますか？（y で終了）」 |
| Quit while saves fail | `q` or Ctrl+C with 「保存できません」 showing | the only confirmation in the app, on the help line | `y` quits, any other key stays | 「保存できていない変更があります。終了しますか？（y で終了）」 |
| Day data unreadable | a day file is corrupt or of a newer format at start | the TUI does not start: stderr and exit 1; the file is never overwritten | fix or move the file | 「error: <path> を読めませんでした（<cause>）。ファイルはそのままです。」 |
| Too small | under 60 × 18 | T1-e | widen the terminal | 「端末が小さすぎます」「60×18 以上に広げてください」 |

## Feedback and loading

| Rule | Value |
|---|---|
| Thinking indicator | nothing under 300 ms; from 300 ms the header word and the chat row; both go when the reply arrives. Why 300 ms: most calls take 1–4 s, so the row shows for nearly every message without flashing for a quick one |
| Long waits | at 10 s the chat row names Esc; at the 30 s† timeout the Call failed state |
| Cancel | Esc while the model works on the owner's message; otherwise Esc clears the input. A check-in's call cannot be cancelled: it posts nothing until it is done |
| Replies | shown whole when complete, never streamed (rejected: streaming, which would need the reply in a separate call from the structured changes) |
| Changes | applied at once, shown as one change line per message or key action; undo with Ctrl+Z or `u` |
| Destructive actions | delete (`x`) is undoable, so it never asks; the only confirmation is quitting while saves fail |
| Bell | one BEL for each unprompted message — note, question, or catch-up — when it is posted; none for replies, change lines, errors, or notices |
| New messages | when the chat is scrolled up, new rows do not move it; its bottom border shows 「新着 N（End で最新へ）」, and End returns to the newest, which turns following back on |
| 新着 divider | a 「── ここから新着 ──」 row above the first message posted since the owner's last key press; the next key press removes it. Why: coming back to the terminal, the owner sees at once what the secretary said meanwhile |
| Error placement | by severity: a form's input → the line under its fields; a one-off failure → one エラー row in the chat; an ongoing condition → a header word, plus one row explaining it when it starts; a refusal to start → stderr and exit 1. Error and システム rows are never sent to the model |

## Forms and validation

- The task form (T2) validates on Enter; once an error is showing, every keystroke
  re-validates, so it clears as soon as the field is fixed. Why: the form has three
  fields, so checking on Enter interrupts nothing, and live re-checking spares a second
  Enter.
- A failed Enter moves focus to the first invalid field; nothing typed is ever cleared.
- Required: the title; the time when the kind is 締切 or 予定. Nothing is marked
  optional — the time field is skipped while the kind is 時刻なし.
- Error lines say what to fix, with the limit in it: ux-flows.md T2.

## Motion

- None: no animation, spinner, blinking, or transition. Overlays appear and disappear
  in one frame.
- The screen redraws on an event — a key, a reply, a tick that changed something, a
  resize — and once a minute for the clock, which shows minutes only.
- Reduced motion: nothing to reduce. The terminal's own cursor blink is the terminal's
  setting.

## Language and copy

- UI language: Japanese only — labels, the help line, T3, notices, errors, the CLI's
  output, and `--help`. No i18n layer. Rejected: English (two languages on one screen)
  and a terse plain style (reads curt).
- Formatting: times as 24-hour HH:MM; the header date as M/D(曜); ISO 8601 dates in
  `--json`; character counts with 字 (「112/600字」).
- Register of the app's own sentences: です・ます, plain, no exclamation marks, no emoji,
  so the app is never mistaken for the secretary. The secretary's lines follow the
  owner's instructions, and the app never rewrites them.
- Speaker labels in the chat: Bunshin, あなた, 変更 (change lines), システム (the app's
  notices), エラー.
- Errors: what happened, the cause in parentheses, then the next step as a sentence —
  「保存できませんでした（ディスクの空きが足りません）。…」.
- Help line and T3: key, then a verb or noun — 「Space 完了」「b 受信箱」.

| Use | Don't use | For |
|---|---|---|
| タスク | TODO, やること | an item in the list |
| 締切 | 期限, デッドライン | a deadline task, and its time |
| 予定 | イベント, アポ | an appointment task |
| 持ち越し / 持ち越す | 繰越, 延期 | moving a leftover to today |
| やめる | 削除, キャンセル | dropping a task (it stays, marked 「[-]」) |
| 削除 | やめる | removing a task added by mistake (`x`) |
| 取り消し | 元に戻す, アンドゥ | undo |
| 見回り | チェック, 巡回 | a check-in, and the planned next look |
| お知らせ / 質問 | 通知, メッセージ | the two kinds of unprompted message |
| 受信箱 | 通知一覧, インボックス | T4 |
| ミュート | 通知オフ, おやすみ | the mute |
| 時間外 | 夜間, 営業時間外 | outside active hours |
| 指示文 | プロンプト, システムプロンプト | the owner's instructions |
| 既定 | デフォルト | the default instructions |

## Accessibility targets

The web criteria are carried over where a terminal has an equivalent; pointer and
reflow criteria have none.

| Target | This product |
|---|---|
| Level | WCAG 2.2 AA where a terminal applies it: everything is text, every function by keyboard, nothing by color alone, focus visible |
| Focus | the focused region has a heavy border and a bold title; the selected row has `>` and reverse video; the cursor shows at the input's insertion point when the input has focus, and otherwise is hidden and parked at the start of the selected row, which is where VoiceOver in Terminal reads |
| Minimum target size | not applicable: no pointer |
| Keyboard | every action has a key, named in the help line and T3 (the template's rule); chords only Ctrl+C, Ctrl+Z, Shift+Tab; full-width keys read as ASCII (Platform conventions) |
| Announcements | an unprompted message rings the bell; ongoing states are words in the header, never only a mark |
| Color use | never the only carrier: `[ ]` `[x]` `[-]` `[>]`, the tag lines, and header words carry every state; the screen reads the same in a monochrome terminal |
| Text scaling | the terminal's font size; the layout holds down to 60 × 18 (ux-flows.md §2) |
| Contrast | text in the default color, red, blue, or magenta only, and green only on the focused border — measured on the owner's dark theme and a light one in `design-direction.md` › Measured contrast (all pass); no faint (DIM) text anywhere |
| Motion | none (Motion) |

Verification for the implementing session: TestBackend tests check that every state's
text is present without styles (the monochrome rule). The runs that need a real
terminal are the owner's: VoiceOver in Terminal.app over F1 and F2, a monochrome
terminal profile, and the IME in kana and full-width alphanumeric modes (an agent never
runs `bunshin tui`).

## App-type rules

- **Show what it is doing:** 「考え中…」 and 「見回り中…」 in the header while the model
  works, and every unprompted message's tag line says why it spoke. Why: a secretary that
  acts on its own must make its reasons visible to be trusted.
- **A stop control:** Esc cancels the owner's call (Feedback and loading).
- **Output is a proposal the owner can undo:** changes from words are applied, shown,
  and undoable rather than confirmed one by one. Why: confirming every change would make
  plain words slower than keys.
- **Not everything through chat:** every task operation is also a key. Why: the
  secretary feel is in the voice, not in forcing commands into sentences.
- **Uncertainty is said, not guessed:** an unclear task reference gets a question, not a
  change (requirements §3.3).

## Non-goals

| Not doing | Why | Covered instead by |
|---|---|---|
| Mouse support | it would take the terminal's text selection away | the key table |
| Animation, spinners | the terminal stays still; words say the state | header words |
| Streaming replies | short replies, structured output | whole replies within 1–4 s |
| RGB or 256-color values | the owner's theme decides how the 16 named colors look | `design-direction.md` |
| A second language | one owner, who reads Japanese | — |

## Open items

- None. The Japanese user-facing wording, an exception to AGENTS.md's English-only rule
  for code and documents, is recorded in the `deciding-architecture` skill (2026-10-02)
  and stated in `AGENTS.md` › "Important Reminders".

## Decision log

| Date | Decided | Rejected options | Why |
|---|---|---|---|
| 2026-10-02 | Thinking shown as still text after 300 ms; 「まだ考えています（Esc で中止）」 at 10 s; Esc cancels the owner's call | a spinner; no cancel | still text reads to a screen reader and keeps the screen calm; a slow call must be stoppable |
| 2026-10-02 | One bell for every unprompted message, catch-ups included | questions only; never | a deadline note matters as much as a question when the terminal is behind other windows |
| 2026-10-02 | Japanese only, です・ます for the app's own sentences | short plain style; English | the conversation is Japanese; a neutral polite voice keeps the app apart from the secretary |
| 2026-10-02 | Replies shown whole | streaming | replies are ≤ 200 chars in 1–4 s and arrive in the same structured output as the changes |
| 2026-10-02 | Set from the inputs, not asked: no mouse capture; full-width keys read as ASCII; ambiguous width counted as one; a 新着 divider and a 新着 count while scrolled up; quitting while saves fail asks once; an unreadable day file stops the TUI and is never overwritten; the speaker label システム for the app's notices | — | each follows from a requirement or a template rule; listed for the owner's review |
| 2026-10-02 | Color: the terminal's 16 named colors, text only in red, blue, and magenta, green only on the focused border; no faint text (`design-direction.md`) | monochrome with modifiers; a fixed RGB palette | the secretary and the errors stand out, and the owner's theme still decides how they look |
