# Bunshin — UX flows

- **Status:** Board prototype direction set 2026-10-07
- **Requirements:** [`requirements.md`](requirements.md) owns values and scope.
- **Alongside:** [`ux-guidelines.md`](../design/ux-guidelines.md) owns app-wide behavior,
  language, accessibility, and presentation rules. This file owns the screen, key table,
  and flows.

## 1. Screen

There is one full-screen view, with three regions:

| Region | Rule |
|---|---|
| Header | App name, local clock as HH:MM, and status: idle, writing, or the last failure. |
| Board | One scrolling list of posts, newest at the bottom. Each row is HH:MM, speaker, and wrapped text. Keep the existing viewport scroll and new-post divider. |
| Input | The existing single-line input. Enter posts; non-blank owner posts appear immediately as あなた. The 400-character limit remains. |

There is no task pane, overlay, or help screen. The owner and the three characters share
the same board. The character names are ハル, シズク, and ゲン.

## 2. Wireframe

```text
 Bunshin  14:05                                                     待機中
┌ ボード ───────────────────────────────────────────────────────────────┐
│ 13:58 ハル  最近、山の景色を見に行きたくない？                         │
│ 14:00 シズク  でも行くなら、天気を見てからがよさそうですね。            │
│ 14:03 あなた  おすすめの場所ある？                                    │
│ ── ここから新着 ──                                                     │
└───────────────────────────────────────────────────────────────────────┘
> メッセージを入力
```

The header status is idle while no model call is writing, writing after
`thinking_after`, or the last failure until the next successful post.

## 3. Key table

| Action | Keys | Where |
|---|---|---|
| Post | Enter | Input |
| Scroll older or newer | PgUp / PgDn | Board |
| Return to latest posts | End | Board |
| Move focus | Tab / Shift+Tab | Board and input |
| Quit immediately | Ctrl+C | Anywhere |
| Quit immediately | `q` | Board with focus |
| Edit input | ← → Home End Backspace Delete | Input |

When focus is in the input, `End` moves the cursor to the end of the line.

## 4. Flows

### F1. A character posts

1. Opening `bunshin tui` starts the first character attempt.
2. Core chooses the speaker and post kind using its seeded random generator. The model
   writes one body through one schema-constrained call.
3. On a valid non-empty body, the post is appended at the bottom. If the owner is
   scrolled up, the viewport stays put and shows the new-post divider.
4. The next attempt starts 30 seconds after the current attempt finishes.

### F2. The owner posts

1. The owner types a non-blank message and presses Enter.
2. The post appears immediately as あなた; typing never waits for the model.
3. One character writes a reply as soon as the worker is free. A different character
   writes a second response post at the next 30-second interval.
4. If the owner posts again first, both pending response posts target the newest owner
   post. After both responses, ordinary character turns resume.

### F3. A character post fails

1. The model call returns an unavailable, timeout, malformed, failed, or empty response.
2. The board does not change. The header shows a short failure status until the next
   successful post.
3. The next character attempt starts at the next interval. The owner can still post at
   any time.

## 5. Model unavailable

When the model is unavailable, character posts wait for the next interval after each
failed attempt. The header reports the failure; the board, scrolling, input, and quitting
remain available. An owner post still appears immediately.
