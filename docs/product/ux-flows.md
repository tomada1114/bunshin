# Bunshin — UX flows and wireframes

- **Status:** Confirmed 2026-10-02
- **Input:** [`requirements.md`](requirements.md) (signed off 2026-10-02, amended the same day). Section
  numbers such as §3.5 point there.
- **Alongside:** [`ux-guidelines.md`](../design/ux-guidelines.md) owns the app-wide rules — feedback timing, how
  errors are shown, motion, the labels' language and tone, accessibility. This file owns
  the screens, their layout and states, the flows, and the key table, and cites the
  guidelines rather than restating them.
- Labels are drawn in Japanese, the language the conversation is held in; the labels'
  language itself is settled in `ux-guidelines.md`. Every wireframe is drawn at its real
  terminal width, counting a Japanese character as two columns.

## 1. Screen inventory

| ID | Screen | Kind | Serves |
|---|---|---|---|
| T1 | Main screen: header, task pane, chat, input, help line | full screen | §3.1–§3.7, §3.10 (count) |
| T2 | Task form (add / edit) | popup over T1 | §3.4 |
| T3 | Help: every key | overlay over T1 | building-tuis: every action named on screen |
| T4 | Inbox | overlay over T1 | §3.10 |
| T5 | Instructions (read-only) | overlay over T1 | §3.8 |
| C1 | `bunshin today [--json]` | stdout / stderr | §3.9 |
| C2 | `bunshin instructions` | stdout / stderr | §3.9 |
| C3 | `bunshin instructions edit` | the owner's editor, then stderr | §3.8, §3.9 |
| C4 | `bunshin tui` refusing to start | stderr | §3.7, building-tuis |

There is no settings screen (the starting values live in code; the instructions are
edited in the owner's editor) and no confirmation dialog: every change is undoable, and
the day is written on every change, so quitting loses nothing.

## 2. Layout of the main screen (T1)

| Region | Rule |
|---|---|
| Header | one row: the app name, the **logical** date (from 00:00 to 03:59 it still shows the previous day) and the clock on the left; status items on the right, in this order, each omitted when it does not apply: 受信箱 N, 保留 N (a message held while typing), 保存できません, ミュート中 〜HH:MM, 時間外（HH:MM から）, 次の見回り HH:MM (— when none is planned), and the model's state: 待機中, 考え中…, or モデル: 使えません |
| Wide (≥ 100 columns) | the task pane takes the left 36 columns at full height; the chat and the input share the right side |
| Narrow (60–99 columns) | the task pane spans the width above the chat, as tall as its rows plus its border, at most 12 rows (10 tasks; beyond that it scrolls with the selection) |
| Too small (< 60 × 18) | only the "too small" message; nothing panics (T1-e) |
| Input | under the chat; one line of text growing to three as the text wraps; its title carries the reply target and a counter against 400 characters |
| Help line | the last row: the keys of the focused region, from core's key table, in the order of §4; in the task pane `? 全キー` comes first so a narrow terminal never cuts it off |
| Focus | Tab and Shift+Tab move between the input and the task pane; the focused region has a heavy border; an overlay or the form takes focus until it closes |

**Task rows.** A selection marker (`>`), the task's number, its status — `[ ]` open,
`[x]` done, `[-]` dropped, `[>]` carried over — a 7-column time field — `10:00` for an
appointment, `〜15:00` for a deadline, blank when untimed — and the title, cut with
`…` when it does not fit. Open tasks come first, timed ones by time; done and dropped
follow (§3.2).

**Chat rows.** The time, an 8-column speaker field — `Bunshin`, `あなた`, `変更` (a
change), `システム` (a notice from the app itself), `エラー` — and the text, wrapped to
the text column. An unprompted message opens with a tag line, `[質問 / <trigger>: <task>]`
or `[お知らせ / <trigger>]`, so its kind and its reason read without color (§3.5). A change
line lists each change with a mark: `+` added, `x` done, `-` dropped, `>` carried over,
`~` edited, and `取り消し:` before an undone set.

## 3. Wireframes

### T1 Main screen

**T1-a — wide, typing an answer to the 14:30 question.** The input has focus; the
14:30 question is under 15 minutes old, so what is typed answers it (§3.10), and the
input's title says so.

```
 Bunshin  10/2(木) 14:31                                       受信箱 1 | 次の見回り 15:00 | 待機中
┌ 今日のタスク ──────── 未完了 3/6 ┐┌ チャット ────────────────────────────────────────────────────┐
│  3 [ ] 〜15:00 資料作成          ││ 09:02 Bunshin  おはよう。昨日は3件完了、残りは「経費精算」。 │
│  4 [ ] 18:00   ジム              ││                持ち越す？ それと今日の予定を教えて。         │
│  5 [ ]         メール返信        ││ 09:03 変更     > 1 経費精算（前日から持ち越し）              │
│  2 [x] 10:00   定例              ││ 09:05 あなた   10時から定例、15時までに資料、18時ジム。      │
│  1 [x]         経費精算          ││                あとメール返信も                              │
│  6 [-]         買い物            ││ 09:05 変更     + 2 10:00 定例（予定）                        │
│                                  ││                + 3 〜15:00 資料作成（締切）                  │
│                                  ││                + 4 18:00 ジム（予定）                        │
│                                  ││                + 5 メール返信                                │
│                                  ││ 09:05 Bunshin  了解。資料は定例のあと、午前中に手をつけると  │
│                                  ││                楽かも                                        │
│                                  ││ 11:12 あなた   定例おわった。経費精算も                      │
│                                  ││ 11:12 変更     x 2 定例 / x 1 経費精算                       │
│                                  ││ 11:12 Bunshin  おつかれ！ 残りは資料、ジム、メール返信だね   │
│                                  ││ 14:30 Bunshin  [質問 / 締切30分前: 資料作成]                 │
│                                  ││                資料、どこまで進んだ？                        │
│                                  ││                                                              │
│                                  ││                                                              │
│                                  ││                                                              │
│                                  ││                                                              │
│                                  │└──────────────────────────────────────────────────────────────┘
│                                  │┏ 入力（14:30 の質問への返事） ━━━━━━━━━━━━━━━━━━━━━━━━ 18/400 ┓
│                                  │┃ 半分くらい。16時には終わりそう_                              ┃
└──────────────────────────────────┘┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
 Enter 送信  Tab タスク欄へ  Esc 消去  ^Z 取り消し  PgUp/PgDn 履歴  ^C 終了
```

**T1-b — narrow (80 × 24), the task pane focused.**

```
 Bunshin  10/2(木) 14:31                   受信箱 1 | 次の見回り 15:00 | 待機中
┏ 今日のタスク ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 未完了 3/6 ┓
┃> 3 [ ] 〜15:00 資料作成                                                      ┃
┃  4 [ ] 18:00   ジム                                                          ┃
┃  5 [ ]         メール返信                                                    ┃
┃  2 [x] 10:00   定例                                                          ┃
┃  1 [x]         経費精算                                                      ┃
┃  6 [-]         買い物                                                        ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
┌ チャット ────────────────────────────────────────────────────────────────────┐
│                + 3 〜15:00 資料作成（締切）                                  │
│                + 4 18:00 ジム（予定）                                        │
│                + 5 メール返信                                                │
│ 09:05 Bunshin  了解。資料は定例のあと、午前中に手をつけると楽かも            │
│ 11:12 あなた   定例おわった。経費精算も                                      │
│ 11:12 変更     x 2 定例 / x 1 経費精算                                       │
│ 11:12 Bunshin  おつかれ！ 残りは資料、ジム、メール返信だね                   │
│ 14:30 Bunshin  [質問 / 締切30分前: 資料作成]                                 │
│                資料、どこまで進んだ？                                        │
└──────────────────────────────────────────────────────────────────────────────┘
┌ 入力 ───────────────────────────────────────────────────────────────── 0/400 ┐
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
 ? 全キー  Space 完了  a 追加  e 編集  d やめる  u 取消  b 受信箱  Tab 入力欄
```

**T1-c — the start of a day, with yesterday's leftovers.** The leftovers sit in their
own block at the top of the task pane until each is carried over or dropped, by key or
by chat; the chat stays usable meanwhile (§3.6).

```
 Bunshin  10/2(木) 09:02                                  次の見回り — | 待機中
┏ 今日のタスク ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 未完了 0/0 ┓
┃  前日の残り 10/1(水) ─ 持ち越すか、やめるか決めてください                    ┃
┃> [ ] 〜17:00 経費精算                                                        ┃
┃  [ ]         請求書の確認                                                    ┃
┃  ──────────────────────────────────────────────────────────────────────────  ┃
┃  （今日のタスクはまだありません）                                            ┃
┃                                                                              ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
┌ チャット ────────────────────────────────────────────────────────────────────┐
│ 09:02 Bunshin  おはよう。昨日は3件完了、2件が残ったよ（経費精算、請求書の確  │
│                認）。                                                        │
│                どうする？ 決まったら今日の予定も教えて。                     │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
┌ 入力 ───────────────────────────────────────────────────────────────── 0/400 ┐
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
 ? 全キー  c 持ち越し  d やめる  C 全部持ち越し  D 全部やめる  Tab 入力欄
```

**T1-d — the first run ever.** No leftovers, no tasks, the default instructions in use.

```
 Bunshin  10/2(木) 09:00                                  次の見回り — | 待機中
┌ 今日のタスク ──────────────────────────────────────────────────── 未完了 0/0 ┐
│  （まだありません ─ 予定を入力欄に書くか、Tab で移って a で追加）            │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
┌ チャット ────────────────────────────────────────────────────────────────────┐
│ 09:00 システム 指示文は既定のものを使っています。自分の言葉で書くには、      │
│                端末で bunshin instructions edit を実行してください。         │
│ 09:00 Bunshin  はじめまして。今日の予定を教えて。                            │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
┏ 入力 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 0/400 ┓
┃ _                                                                            ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
 Enter 送信  Tab タスク欄へ  Esc 消去  ^Z 取り消し  PgUp/PgDn 履歴  ^C 終了
```

**T1-e — a terminal smaller than 60 × 18.**

```
┌────────────────────────────────────────────────┐
│                                                │
│                                                │
│                                                │
│                                                │
│              端末が小さすぎます                │
│         60×18 以上に広げてください             │
│                                                │
│                                                │
│                                                │
│                                                │
└────────────────────────────────────────────────┘
```

**Header variants** (80 columns): normal; the model working; a message held while
typing; muted; outside active hours; the model unavailable; a save failing; after
midnight, before the day turns at 04:00.

```
 Bunshin  10/2(木) 14:31                   受信箱 1 | 次の見回り 15:00 | 待機中
 Bunshin  10/2(木) 14:31                  受信箱 1 | 次の見回り 15:00 | 考え中…
 Bunshin  10/2(木) 14:31          受信箱 1 | 保留 1 | 次の見回り 15:00 | 待機中
 Bunshin  10/2(木) 14:31         ミュート中 〜15:31 | 次の見回り 15:00 | 待機中
 Bunshin  10/2(木) 23:10       時間外（08:00 から） | 次の見回り 08:00 | 待機中
 Bunshin  10/2(木) 14:31                      次の見回り — | モデル: 使えません
 Bunshin  10/2(木) 14:31             保存できません | 次の見回り 15:00 | 待機中
 Bunshin  10/1(水) 01:30                          時間外（08:00 から） | 待機中
```

**Chat variants.** The thinking row, and the same row after 10 s; a cancelled call; a
note and a question from a check-in; the catch-up after the screen was closed; the
evening review; a change and its undo; an edit by key; a reply asking which task; a
change the core refused; a mute; a model failure; the task limit; and the 新着 divider
above what arrived since the last key press (timings and rules: `ux-guidelines.md`,
Feedback and loading).

```
14:32 Bunshin  …
14:32 Bunshin  まだ考えています（Esc で中止）
14:33 システム 中止しました。変更はありません。
14:40 Bunshin  [お知らせ / 予定した見回り]
               午後はメール返信から片付けると楽そう
15:00 Bunshin  [質問 / 締切: 資料作成]
               資料の締切の時間だよ。終わった？
17:05 Bunshin  [質問 / 閉じている間に]
               資料作成の締切（15:00）を過ぎてたよ。どうなった？
18:00 Bunshin  [質問 / 夕方の振り返り]
               今日は4件完了。メール返信が残ってるけど、今日やる？ 明日に回
               す？
14:41 あなた   資料終わった
14:41 変更     x 3 資料作成
14:42 変更     取り消し: x 3 資料作成
14:45 変更     ~ 4 18:00 → 19:00 ジム
14:50 Bunshin  どっちのこと？ 3 資料作成 / 5 メール返信
14:52 Bunshin  「7」というタスクは見つからなかったよ。
15:31 変更     ミュート 〜16:31（m で解除）
16:31 システム ミュートが終わりました。
14:55 エラー   うまく読み取れませんでした。もう一度送ってください。
14:56 エラー   今日のタスクは50件までです。

         ── ここから新着 ──         
15:40 Bunshin  [お知らせ / 予定した見回り]
               メール返信、夕方までに片付けると気が楽かも
```

**When the model is unavailable** (§4): one notice naming the cause and the owner's
step, deadline notes as fixed sentences, and a notice when the model is back.

```
09:00 システム オンデバイスのモデルを使えません: fm コマンドが見つかりません
               （macOS 27 以降の Apple シリコン Mac が必要です）。タスクの操
               作はこのまま使えます。
09:00 システム オンデバイスのモデルを使えません: 利用規約が未承認です。端末
               で sudo fm license を実行してください。
09:00 システム オンデバイスのモデルの準備ができていません（ダウンロード中か
               もしれません）。10分ごとに確かめます。
15:00 Bunshin  [お知らせ / 締切: 資料作成]
               資料作成（〜15:00）の締切を過ぎました。
15:10 システム モデルが使えるようになりました。
```

### T2 Task form

Opened with `a` (empty, kind 時刻なし) or `e` / Enter (filled from the selected task).
The time field is skipped while the kind is 時刻なし. Errors appear on the line under
the fields, and Enter does nothing until they are fixed:

- empty title, or over 80 characters: 「タイトルは1〜80字で入力してください」
- a deadline or an appointment without a time: 「締切と予定には時刻が必要です」
- a time that is not HH:MM between 00:00 and 23:59: 「時刻は 00:00〜23:59 で入力してください」

At the 50-task limit the form does not open; the chat shows the limit error (T1 chat
variants).

```
┏ タスクを追加 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃                                                  ┃
┃ タイトル  [資料作成_____________________]        ┃
┃ 種類      ( ) 時刻なし  (*) 締切  ( ) 予定       ┃
┃ 時刻      [15:00]                                ┃
┃                                                  ┃
┃ （時刻は 00:00〜23:59 で入力してください）       ┃
┃                                                  ┃
┃ Enter 保存  Tab 次の項目  ←→ 種類  Esc やめる    ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

### T3 Help

```
┏ キー操作（? または Esc で閉じる） ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃                                                                              ┃
┃ どこでも     ^C 終了   ^Z 取り消し   Tab/Shift+Tab 入力欄⇔タスク欄           ┃
┃              PgUp/PgDn チャットをさかのぼる   End 最新へ                     ┃
┃ 入力欄       Enter 送信   Esc 消去（考え中は中止）                           ┃
┃              ←→ Home End Backspace Delete                                    ┃
┃ タスク欄     ↑↓ (k/j) 選択   Space 完了⇔未完了   d やめる⇔戻す               ┃
┃              a 追加   e/Enter 編集   x 削除   u 取り消し   i 入力欄へ        ┃
┃              m ミュート60分⇔解除   b 受信箱   p 指示文                       ┃
┃              ? この画面   q 終了                                             ┃
┃ 前日の残り   c 持ち越し   d やめる   C 全部持ち越し   D 全部やめる           ┃
┃ 受信箱       Enter 返事する／了解   x 閉じる   X お知らせを全部了解          ┃
┃              g そのタスクへ   Esc/b 戻る                                     ┃
┃ フォーム     Enter 保存   Tab 次の項目   ←→ 種類   Esc やめる                ┃
┃                                                                              ┃
┃ キーは英数入力で押してください（全角英数のままでも使えます）                 ┃
┃ 表示: [ ] 未完了  [x] 完了  [-] やめた  [>] 持ち越し済み                     ┃
┃       10:00 予定（その時刻に始まる）  〜15:00 締切（その時刻までに）         ┃
┃                                                                              ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

### T4 Inbox

Opened with `b`; lists today's open items, newest first (§3.10). Enter on a question
closes the inbox and puts the focus in the input with the question as the reply target;
Enter on a note acknowledges it.

```
┏ 受信箱 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 未対応 3 ┓
┃                                                                              ┃
┃ > 14:30 [質問 / 締切30分前: 資料作成]                                        ┃
┃         資料、どこまで進んだ？                                               ┃
┃   13:10 [お知らせ / 予定した見回り]                                          ┃
┃         午後はメール返信から片付けると楽そう                                 ┃
┃   11:40 [質問 / 予定した見回り: メール返信]                                  ┃
┃         メール返信、今日中にやる？                                           ┃
┃                                                                              ┃
┃   ── 未対応は今日の分だけ。15分反応がないと「無視」として秘書に伝わります    ┃
┃                                                                              ┃
┃ Enter 返事する／了解  x 閉じる  X お知らせを全部了解  g タスクへ  Esc 戻る   ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

```
┏ 受信箱 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 未対応 0 ┓
┃                                                                              ┃
┃ 対応が必要なものはありません。                                               ┃
┃                                                                              ┃
┃                                                                              ┃
┃ Esc 戻る                                                                     ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

### T5 Instructions

Opened with `p`. Read-only; when the default is in use, it says why (no file, an empty
file, or over 600 characters).

```
┏ 指示文（読み取り専用） ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 112/600字 ┓
┃                                                                              ┃
┃ あなたは私の分身で、秘書として私の一日を見守る。口調はタメ口で、短く。       ┃
┃ 平日は9〜18時が仕事。午前は集中したいので、急ぎでなければ午後に声をかけて。  ┃
┃ 締切の前は少ししつこくていい。終わったら一言ほめて。                         ┃
┃                                                                              ┃
┃ ファイル: ~/Library/Application Support/<bundle-id>/instructions.md          ┃
┃ 編集: 端末で bunshin instructions edit（保存後、次の呼び出しから反映）       ┃
┃                                                                              ┃
┃ ↑↓ スクロール  Esc/p 閉じる                                                  ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

```
┏ 指示文（読み取り専用） ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 既定を使用中 ┓
┃                                                                              ┃
┃ ファイルの指示文が600字を超えている（612字）ため、既定を使っています。       ┃
┃                                                                              ┃
┃ あなたはユーザーの秘書です。ユーザーの一日を見守り、短く声をかけます。       ┃
┃                                                                              ┃
┃ ファイル: ~/Library/Application Support/<bundle-id>/instructions.md          ┃
┃ ↑↓ スクロール  Esc/p 閉じる                                                  ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

### C1 `bunshin today`

The same order and marks as the task pane. An empty day prints nothing and exits 0.
Between 00:00 and 03:59 it shows the previous day, as the screen does.

```
$ bunshin today
3  [ ]  〜15:00  資料作成
4  [ ]  18:00    ジム
5  [ ]           メール返信
2  [x]  10:00    定例
1  [x]           経費精算
6  [-]           買い物
```

`--json` prints one object carrying the format version; its exact fields are settled with
`designing-clis` with the architecture:

```
$ bunshin today --json
{"format":1,"date":"2026-10-02","tasks":[{"number":3,"title":"資料作成","kind":"deadline","time":"15:00","status":"open"}, …]}
```

When the day's data cannot be read, stderr gets one line and the exit code is 1:

```
$ bunshin today
error: 今日のデータを読めませんでした（ファイルの形式が新しすぎます）
```

### C2 `bunshin instructions`

The text in use goes to stdout, so it can be piped; where it came from goes to stderr.

```
$ bunshin instructions
あなたは私の分身で、秘書として私の一日を見守る。口調はタメ口で、短く。
平日は9〜18時が仕事。午前は集中したいので、急ぎでなければ午後に声をかけて。
締切の前は少ししつこくていい。終わったら一言ほめて。
file: ~/Library/Application Support/<bundle-id>/instructions.md (112/600)
```

With no file, the default goes to stdout and stderr says
`using the default: no file at ~/Library/Application Support/<bundle-id>/instructions.md`.

### C3 `bunshin instructions edit`

Writes the default into the file when there is none, opens it in `$VISUAL`, else
`$EDITOR`, waits for the editor to close, then checks the result:

```
$ bunshin instructions edit
指示文を保存しました（112/600字）。次の呼び出しから使われます。

$ bunshin instructions edit
error: 指示文が600字を超えています（612字）。直すまでは既定の指示文が使われます。

$ bunshin instructions edit
error: エディタが設定されていません。VISUAL か EDITOR を設定するか、次のファイルを直接編集してください: ~/Library/Application Support/<bundle-id>/instructions.md
```

The first exits 0; the others exit 1. An editor that exits non-zero is reported with its
exit code, and the file is checked as it was left.

### C4 `bunshin tui` refusing to start

Both exit 1 before touching the terminal or the data.

```
$ bunshin tui < /dev/null
error: bunshin tui は端末の中で実行してください

$ bunshin tui
error: bunshin tui はすでに起動しています（PID 4821）
```

## 4. Key table

A proposal for core's key table: one action per row, and the help line and T3 are built
from it.

| Action | Keys | Where | Notes |
|---|---|---|---|
| Quit | Ctrl+C; `q` | anywhere; task pane | the day is already written |
| Undo | Ctrl+Z; `u` | anywhere; task pane | up to 20† sets back (§3.4) |
| Move focus | Tab / Shift+Tab; `i` | anywhere but overlays; task pane | `i` goes to the input, for Vim habits |
| Scroll the chat | PgUp / PgDn, End | anywhere | End returns to the newest |
| Send | Enter | input | an empty input sends nothing; while the model is busy the message waits (§3.1) |
| Clear, or cancel | Esc | input | while the model works on the owner's message, cancels it (`ux-guidelines.md`); otherwise clears the input and the reply target |
| Edit text | ← → Home End Backspace Delete | input | the terminal composes Japanese input |
| Select | ↑ ↓, `k` `j` | task pane, inbox | |
| Done ⇔ open | Space | task pane | |
| Drop ⇔ open | `d` | task pane | |
| Add | `a` | task pane | opens T2 |
| Edit | `e`, Enter | task pane | opens T2 filled |
| Delete | `x` | task pane | no confirmation; undoable |
| Mute ⇔ unmute | `m` | task pane | 60 min† (§3.5) |
| Inbox | `b` | task pane | opens T4 |
| Instructions | `p` | task pane | opens T5 |
| Help | `?` | task pane | opens T3 |
| Carry over / drop a leftover | `c` / `d` | leftovers block | |
| All leftovers | `C` / `D` | leftovers block | |
| Answer or acknowledge | Enter | inbox | a question → the input, with it as the reply target; a note → acknowledged |
| Close an item | `x` | inbox | a note → acknowledged; a question → dismissed |
| Acknowledge every note | `X` | inbox | questions stay |
| Go to its task | `g` | inbox | closes the inbox and selects the task |
| Close an overlay | Esc, or the key that opened it | T3, T4, T5 | |
| Save / next field / kind / cancel | Enter / Tab / ← → / Esc | T2 | |

## 5. Flows

### F1 The start of a day (§3.6)

1. The owner opens `bunshin tui` — or, with the screen left open overnight, presses a key
   after 04:00.
2. If the last day on record left open deadline or untimed tasks, they appear in the
   leftovers block (T1-c) and Bunshin's first message gives yesterday's record and asks
   about them.
3. The owner settles each by key (`c` / `d` / `C` / `D`) or in words (「全部持ち越し」); each
   decision is a change line, and the block closes when it is empty.
4. Bunshin asks for today's plan; the owner writes it; change lines show the new tasks.

```
[open, or first key after 04:00]
        |
        v
  [leftovers?] --no--> [Bunshin asks for the plan] --> [owner writes the plan] --> [change lines]
        | yes                     ^
        v                         |
  [block + question] --> [c/d/C/D or words] --> [block empty]
        |
        v (model unavailable)
  [notice once; no greeting] --> [leftovers by key; tasks by T2]
```

A misread plan is undone with Ctrl+Z (or `u`), or fixed in T2 (F3).

### F2 Telling it what got done (§3.3)

```
[types 「資料終わった」] -> [Enter] -> [考え中…] -> [change line: x 3 資料作成] + [reply]
                                         |
             +---------------------------+---------------------------+
             v                           v                           v
   [unclear which task]        [change the core refuses]      [model call fails]
   reply asks 「どっち？」       reply says what was wrong      エラー: うまく読み取れ
   nothing changes             nothing changes                ませんでした — resend

[wrong change] -> [Ctrl+Z or u] -> [change line: 取り消し: x 3 資料作成]
```

### F3 Fixing a misread by key (§3.4)

Tab to the task pane → select the task → `e` → T2 → change the kind with ← →, the time,
or the title → Enter → a change line (`~ 4 18:00 → 19:00 ジム`). A validation error keeps
the form open; Esc leaves it unchanged.

### F4 A check-in (§3.5)

```
[tick, every 60 s†]
   |
   v
[any trigger?] --no--> (nothing; no model call)
   | yes
   v
[muted?] --yes--> [held until unmuted or the mute ends] --+
   | no                                                   |
   v                                                      |
[outside 08:00–22:00†?] --yes--> [held until 08:00] ------+
   | no                                                   |
   v                                                      v
[< 5 min† since the last one?] --yes--> [wait]     [one catch-up message]
   | no
   v
[owner typing?] --yes--> [header: 保留 1; wait for send or clear]
   | no
   v
[model: silent / note / question, its task, next look]
   |-- silent --> (nothing posted; next look planned)
   |-- same task within 30 min† --> (suppressed, recorded)
   v
[chat row with its tag + bell + inbox]
   v
[reaction: answered / acknowledged / dismissed / task closed / muted; ignored at 15 min†]
```

When the model is unavailable, or a call for a deadline trigger fails, the deadline note
is posted as its fixed sentence; any other trigger waits for the model (§3.5).

### F5 Answering from the inbox (§3.10)

`b` → T4 → select the 14:30 question → Enter → the input, titled 「入力（14:30 の質問への
返事）」 → type → Enter → the item is answered and leaves the inbox. On a note, Enter
acknowledges it; `x` closes an item; `X` acknowledges every note; `g` jumps to the task.

### F6 Muting (§3.5)

`m`, or 「1時間静かにして」 in chat → a change line `ミュート 〜16:31` and the header item →
triggers are held → `m` again, or the time passes → 「ミュートが終わりました。」 and, if
anything was held, one catch-up message.

### F7 The evening review (§3.6)

At 18:00† (inside active hours, not muted) Bunshin posts a question with the day's
summary. The owner answers in words (「メールは明日」); nothing changes without them. What
is still open becomes tomorrow's leftovers.

### F8 Coming back: reopening or waking (§3.7)

```
[screen opens, or the tick resumes after > 5 min†]
   |
   v
[same day?] --no--> [F1; held triggers of the previous day are dropped]
   | yes
   v
[triggers came due meanwhile?] --yes--> [one catch-up message, a note or a question]
   | no
   v
(nothing)
```

### F9 Editing the instructions (§3.8)

In a shell: `bunshin instructions edit` → the editor → save and close → C3's result line.
The next model call uses the new text; in the TUI, `p` shows it. Over the limit, the
default is used and the chat says so once, until the file changes.

### F10 The model unavailable (§4)

At the first model call (or when the screen opens), the cause is found → one notice
naming it and the owner's step → keys, the inbox, and the leftovers keep working →
availability is checked again every 10 min† → 「モデルが使えるようになりました。」, and
the waiting day start or review goes ahead.

## 6. States per screen

| Screen | Populated | Empty | Working | Error | Other |
|---|---|---|---|---|---|
| T1 | T1-a, T1-b | T1-d (first run); an empty day after the leftovers | 考え中… in the header and the chat | error rows in the chat; 保存できません in the header; the model unavailable | the day start (T1-c), muted, held while typing, outside active hours, too small (T1-e) |
| T2 | filled from a task | new | — | the line under the fields | the limit: the form does not open |
| T3 | always the same | — | — | — | — |
| T4 | open items | 「対応が必要なものはありません。」 | — | — | — |
| T5 | the file's text | no file → the default, and why | — | over the limit → the default, and why | an empty file → the default |
| C1 | the list | prints nothing | — | unreadable data → exit 1 | `--json` |
| C2 | the file's text | the default, and why | — | — | — |
| C3 | saved | created from the default | — | over the limit; no editor; the editor failed | — |
| C4 | — | — | — | not a terminal; already running | — |

## 7. Rules this file leaves to `ux-guidelines.md`

When the thinking state shows and how long the bell is; where each kind of error goes
(a chat row, the header, the form's line) and its wording; the labels' language and the
tone of the app's own sentences (not the model's, which the instructions set);
motion — this file assumes none: no spinner, no animation; focus visibility beyond
the heavy border; how the screen reads to a screen reader; and which characters the
chrome may use, given that Japanese text is double-width and some symbols (arrows, box
drawing) are ambiguous-width.
