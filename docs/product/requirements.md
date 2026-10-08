# Bunshin — Board prototype requirements

- **Status:** Updated for the board prototype 2026-10-07
- **Platform and stack:** one Rust binary with a full-screen terminal view. On macOS,
  characters use Apple's on-device foundation model through the fm command. On Linux,
  the app builds and reports the model unavailable.
- Values marked † are starting values held in core and adjusted through use.

## 1. Overview

Bunshin is a prototype bulletin board in a terminal pane. Three fixed characters — ハル,
シズク, and ゲン — talk in one shared list, roughly every 30 seconds. The owner can join
by posting in Japanese; their post appears at once and characters respond with posts.
The purpose
is to check that this conversation loop works end to end, not to build a complete
assistant.

## 2. Scope

### MVP

- One in-memory board with three fixed characters and one owner voice, あなた.
- One character post about every 30 seconds, selected by core; the model writes only the
  post body.
- Owner posts appear immediately and receive two character posts in response.
- One terminal screen with a header, scrolling board, and input line.
- The existing on-device model adapter and single worker.

### Later

No later work is selected for this prototype.

### Non-goals

- **Network access.** The prototype calls only the on-device fm command.
- **Acting outside the app.** No action is taken on the owner's behalf.
- **Built-in voice input or speech.**
- **Accounts, multiple users, or syncing between Macs.**
- **Persistence, history, export, or import.**
- **Editable or configurable characters or topics.**
- **Threads, mentions, emoji reactions or likes as separate post controls; editing or deleting posts.**
- **Notifications, a terminal bell, or other sound.**

## 3. Requirements

| ID | Requirement |
|---|---|
| R1 | Add one character post every 30 s†. Measure the interval from the finish of the previous attempt, whether it succeeded or failed, so a slow model call never stacks attempts. Start the first attempt as soon as the TUI opens. |
| R2 | Core uses a seeded pseudo-random draw to choose the speaker and post kind. The starting weights† are Reply 50%, Chime in 25%, and New topic 25%. A reply targets the latest post; a chime-in reacts to the last two posts; a new topic addresses everyone. |
| R3 | A character never posts twice in a row. An empty board or an unsatisfied kind falls back to New topic. |
| R4 | A new topic draws one concrete, casual subject uniformly from 40 compiled-in subjects (for example コンビニの新作スイーツ, 休日の朝ごはん, 最近ハマってるゲーム), skipping the subjects of the last 8† successful New topics. No subject is political or news. The New topic prompt names the subject, asks for one personal experience and an easy question to the others, and carries no earlier posts, so the old conversation does not leak into it. |
| R5 | Enter on non-blank input immediately adds an owner post labelled あなた. Empty or whitespace-only input is ignored. The existing 400-character input limit remains. |
| R6 | After an owner post, one random character writes a reply as soon as the model worker is free, without waiting for the interval. The interval restarts when that reply attempt finishes. |
| R7 | At the next interval, a different character writes a second response post about the same owner post. Normal turn selection then resumes; no topic change is forced. |
| R8 | Every owner post starts a fresh two-turn response cycle for the newest owner post. A newer post resets that cycle; an in-flight character result for an older target is discarded when its call returns, then the first response turn starts as soon as the worker is free and the different character's second turn starts at the next interval. Responses already on the board remain unchanged and do not count toward the new cycle. Owner posts are never refused. |
| R9 | Use one fm respond call per character post through the existing LanguageModel port and worker, asking for plain text with no schema. Instructions combine the speaker's persona and casual-board rules: one or two Japanese sentences as that character, at most 60 characters†, with no name, quotes, or preamble; name a concrete product or experience; no politics or news; do not copy earlier wording. The prompt contains the latest 8 posts† under これまでの流れ as 名前: 本文 lines, the owner named ユーザー (あなた stays the display name), followed by the selected turn instruction. An owner turn quotes the owner's post and asks the character to answer its content directly and to follow a requested change of subject. Personas: ハル is optimistic and curious; シズク is a cheerful, down-to-earth tsukkomi; ゲン is a laid-back trivia lover. Keep the existing 30 s model timeout. |
| R10 | Clean a returned body before display: unwrap a `{"body": …}` object, cut everything from a code fence, a model control marker, or a brace onward, join lines, and remove a leading speaker name, quotes wrapping the whole body, and an unmatched closing quote. Then trim it and cut it to 120 characters† for display. An empty body is a failure. |
| R11 | On an unavailable model, timeout, malformed response, failed call, or empty body, skip the turn, add no post, and show a short failure status in the header. The next attempt is at the next interval. The owner can always post. |
| R12 | The screen has a header, board, and input. The header shows the app name, HH:MM, and idle, writing, or the last failure. Writing takes precedence after `thinking_after`; otherwise retain the latest failure until a successful post clears it. A failed attempt replaces the retained failure. The board is a scrolling list, newest at the bottom; rows show HH:MM, speaker, and wrapped text. Keep the existing scroll behavior and new-post divider. There is no task pane, overlay, or bell. Enter posts; the existing quit key exits immediately. |
| R13 | Posts stay in memory only, with a maximum of 200†; drop the oldest when full. Quitting loses the board. Nothing is written except existing log files. Do not read or delete the earlier app data directory. |
| R14 | The only subcommand is tui. today and instructions are removed and return a clap usage error (exit 2). |

## 4. Model-unavailable rule

A missing or unavailable model prevents character posts, but does not disable the board or
the owner's input. A failed attempt leaves the board unchanged and appears in the header
until a successful post. The next attempt follows the normal interval; no retry is stacked
behind a slow call.

## 5. Data

The board contains ordered posts with a speaker, body, and local display time. It exists
only in memory and holds at most 200† posts. The app continues to write its existing log
files; it does not read, write, or delete earlier day data.

## 6. Open questions

None for this prototype. Values marked † are starting values to adjust through use.

## 7. Decision log

The entries dated 2026-10-02 record the former secretary design. The 2026-10-07 entry
records the prototype direction that replaces it.

- 2026-10-02 Name Bunshin, slug `bunshin` (rejected: Hisho, Aide).
- 2026-10-02 The secretary runs only while `bunshin tui` is open; a terminal bell when it
  speaks; one catch-up summary on reopen (rejected: TUI plus macOS notifications, because
  it adds an OS integration and a permission prompt to the MVP; a launchd daemon with the
  TUI as a window onto it, because the process model, shared state, and install and
  uninstall would roughly double the MVP).
- 2026-10-02 Tasks change from plain words through the model, shown at once and
  undoable, and by key (rejected: plain words only, because a misread could only be fixed
  by more chat with a 3B model; commands only, because it loses what makes it a
  secretary).
- 2026-10-02 The secretary keeps today's tasks with optional clock times (rejected: an
  untimed list, because nudges would be vague; multi-day tasks with due dates, because
  date parsing and the views it needs grow the MVP).
- 2026-10-02 Check-ins are triggered before and after a deadline, by the model's own
  planned check, and at the day's start and end (rejected: a heads-up 10 min before an
  appointment — the owner left it out).
- 2026-10-02 The model decides whether to speak; the core keeps only a 5-minute gap
  between unprompted messages, with no hourly cap and no guaranteed delivery of
  time-bound notices (rejected: a 15-minute gap, 3 an hour, and guaranteed time-bound
  notices; a 30-minute gap and 2 an hour).
- 2026-10-02 The day turns at 04:00; leftovers are offered at the next day start, to carry
  over without their time or to drop (rejected: carrying them over silently, because
  undone tasks pile up; dropping them at the boundary).
- 2026-10-02 The secretary's tone is the owner's to write, as instructions (rejected: a
  fixed casual buddy; a fixed polite secretary).
- 2026-10-02 Appointments are listed with their time but trigger nothing; the model tells
  them from deadlines (rejected: treating every time as a deadline, which would have
  dropped the distinction).
- 2026-10-02 The instructions are one text file, edited in the owner's editor through
  `bunshin instructions edit`, and also carry what the secretary should know about the
  owner (rejected: an editor inside the TUI, because a multi-line editor with Japanese
  input is a large build; rewriting them by chat, because a 3B model rewrites unreliably).
- 2026-10-02 Every day is kept, with no automatic deletion (rejected: 30 days; only the
  previous day).
- 2026-10-02 Subcommands are read-only — `today` and `instructions` (rejected: adding
  and completing tasks from the shell, because the screen would have to merge another
  writer's changes; a one-shot chat from the shell).
- 2026-10-02 Accepted at sign-off, as proposed in round 3: no cloud model; the secretary
  never changes the list on its own; no planning the day for the owner, no priorities,
  tags, projects, or sub-tasks; a one-key 60-minute mute, also proposable by chat; a
  delete key and a 20-step undo; one screen at a time; the day start waits for the first
  key press after 04:00; sleep is caught up like a reopen; open appointments are not
  carried over; owner-only data files; fixed sentences for deadline triggers when the
  model is unavailable, and `sudo fm license` left to the owner.
- 2026-10-02 Amended after the owner's research memo
  (kept outside this repository): the core now guarantees active hours
  (08:00–22:00†) and at most one unprompted message per task in 30 min†, on top of the
  5-minute gap (rejected: active hours only; keeping everything to the model, the round 2
  choice this replaces).
- 2026-10-02 Unprompted messages are notes or questions tied to a task; each has an
  inbox state, the state of the last five goes to the model, and an inbox lists the open
  ones (rejected: recording reactions only as a log; not recording them in the MVP).
- 2026-10-02 The core writes yesterday's record into every call of the day.
- 2026-10-02 Mail moves from Later to Non-goals (the memo records the decision); a hosted
  model moves from Non-goals to Later, behind the same interface as `fm`.
- 2026-10-02 The TUI shows the instructions read-only; editing stays in the editor.
- 2026-10-02 Settled while cutting the issues: the before-deadline fixed sentence is
  「…の締切が近づいています」 with no minutes in it (rejected: 「…の締切まであと30分です」,
  which would drift from the tuned value); the day start and the catch-up at an open are
  held by neither the minimum gap nor the mute, as by no active hours (rejected: the mute
  only; all guards, which could leave a fresh open silent); a reply over 200† characters
  is shown whole (rejected: cutting it with 「…」; treating it as a failed answer).
- 2026-10-07 The board prototype replaces the secretary for this experiment: three fixed
  characters and the owner share one in-memory board, and character turns come from the
  model. The secretary's tasks, check-ins, daily rhythm, inbox, instructions file,
  persisted day files, and read-only subcommands are removed. Rejected: expanding the
  secretary with more screens, stored state, and workflows; the owner wants to test the
  character-and-owner conversation loop end to end first.
- 2026-10-07 After the owner's first `bunshin tui` run felt stiff — abstract policy talk,
  characters ignoring the owner's posts, and JSON fragments in bodies — the board turns
  casual: New topic draws from 40 concrete subjects instead of a category, depth, and
  mood (rejected: keeping the categories with only casual depths and moods, which still
  left the model to invent a vague subject); personas become lighter (rejected: keeping
  シズク as a worrier and ゲン as a summarizer); the owner is ユーザー in prompts, because
  "あなた" also named the character; the context shrinks to 8† posts and New topic
  carries none; the call asks for plain text, because schema-constrained `fm` output ran
  to the context limit in most local runs and leaked JSON into bodies (rejected: keeping
  the schema and only cleaning bodies, which left the timeouts).
