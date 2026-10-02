# Bunshin — Requirements

- **Status:** Signed off 2026-10-02; amended 2026-10-02 after the owner's research memo (§7)
- **Platform and stack:** a terminal app for macOS 27 or later on an Apple silicon Mac,
  built on `tomada1114/rust-template`: one Rust binary, `bunshin`, with clap subcommands
  and a full-screen ratatui view, `bunshin tui`, over a core that does no I/O. The
  secretary thinks with Apple's on-device foundation model, reached through the `fm`
  command-line tool that ships with macOS 27 — no server, no API key, nothing leaves the
  Mac. The template also builds on Linux, where `fm` does not exist; there the model is
  reported unavailable (§4).

Numbers marked † are **starting values**: they live in one tuning type and are adjusted
by using the app, because only the real model on a real day can settle them.

## 1. Overview

- **What it is:** Bunshin (分身, "alter ego") is a secretary that lives in a terminal
  pane. You keep `bunshin tui` open beside your work. At the start of the day you tell it,
  in plain Japanese, what the day holds — 「10時から定例、15時までに資料、18時ジム」 — and
  it keeps today's tasks: some due by a time, some appointments at a time, some with no
  time at all. While the screen is open it looks at the day once a minute. Most minutes
  it stays quiet; when a deadline is near or has passed with the task still open, a
  check-in it planned for itself comes due, or the day starts or winds down, it decides —
  with the model — whether to speak first: a short note or a question in the chat, with a
  terminal bell. It keeps to the hours you are up, never nags about one task twice within
  half an hour, and remembers how you took its last few nudges; what it asks waits in an
  inbox until you answer. How it talks is yours to write: you give it its instructions in
  your own words.
  You answer the way you would answer a person — 「会議終わった」「資料は明日に回す」 — and
  it updates the list, shows what it changed, and lets you undo.
- **What it is not, in this version:** an agent that reaches the outside world. It reads
  no feeds, no Obsidian vault, no mail, no calendar, and calls no API. It is not a
  knowledge source either: the on-device model is small and is used for what it is good
  at — reading what you said, and deciding and phrasing a short nudge.
- **Who it is for:** its developer: one person, at one Mac, who works in the terminal.
  Single user, no accounts.
- **The pain:** today's AI agents wait to be asked. Nobody keeps an eye on the day and
  says 「そろそろこれ片付けたほうがいいんじゃない？」 before a task slips.
- **Core interaction:** telling Bunshin what today holds and what got done, in plain
  words — and being nudged at the right moment without asking for it.
- **The first version works when,** over one working day with `bunshin tui` open:
  yesterday's leftovers and today's plan are settled by chat at the start of the day;
  every task with a deadline is considered for a check-in before and after it; the
  secretary checks in on its own between those, at intervals it chose; marking something
  done is one sentence or one key; every unprompted message shows what triggered it;
  every question it asks waits in the inbox until answered or dismissed; and silencing it
  is one key.

## 2. Scope

### MVP

- **Chat** with the secretary in the TUI (§3.1) — the core interaction happens here.
- **Today's tasks**: untimed, due by a time, or an appointment at a time (§3.2) — what
  the secretary keeps an eye on.
- **Task changes from plain words**, shown at once and undoable (§3.3) — the "tell it
  what got done" half of the core interaction.
- **Direct task editing by key** (§3.4) — so a misread is fixed in one key, not more chat.
- **Proactive check-ins** on a one-minute tick: notes and questions, kept to waking
  hours and spaced per task, with a mute, informed by how the owner reacted to the last
  ones (§3.5) — the "it speaks first" half.
- **The day's rhythm**: the day's start with leftovers and the plan, the evening review
  (§3.6).
- **Persistence and catch-up** across restarts and sleep (§3.7) — closing the screen
  must not lose the day.
- **Instructions you write** (§3.8) — the secretary's role, its tone, and what it should
  know about you, in your own words, readable in the TUI and edited in your editor; what
  makes it your alter ego rather than a stock assistant.
- **Two read-only subcommands** next to the screen (§3.9) — today's tasks for a script
  or a status line, and the instructions file.
- **The inbox and reactions** (§3.10) — what the secretary said or asked that the owner
  has not dealt with, in one place; how the owner reacted is what the next check-in
  learns from.

### Later

- **macOS notifications** when the terminal is not in front — waits because it is an OS
  integration with its own permission prompt; pulled forward if the bell is missed.
- **A background process** (launchd) that keeps the secretary running with the screen
  closed — the process model, state shared between processes, and install and uninstall
  would double the MVP.
- **Multi-day tasks**: due dates beyond today, recurring tasks.
- **Looking back over past days**: a weekly review, search — every day is kept (§3.7),
  so the material is there.
- **Writing from the shell**: `bunshin add` / `bunshin done`, or a one-shot chat — waits
  because two writers need the screen to merge outside changes.
- **Settings for the starting values** (the review time, the gaps) — the † values live
  in code until one needs changing more often than a rebuild.
- **RSS news** digests.
- **Obsidian vault tasks**: read the notes in the owner's vault repository and nudge on
  tasks that stay open.
- **A hosted model** behind the same interface as the on-device one — kept possible,
  not built; it would send the day off the Mac, so it comes only by its own decision.
- **Calendar** (macOS Calendar) as a source of today's appointments.

### Non-goals

- **Network access in this version** — the MVP talks to nothing but `fm`; every
  connection above is a Later item that needs its own decision.
- **Acting outside the app** — it sends nothing, edits no file but its own, and runs no
  command on the user's behalf.
- **A general chatbot or knowledge source** — chat is about the day; answers of fact are
  not something it promises.
- **Built-in voice input or speech** — the OS's dictation into the terminal is enough.
- **Editing or deleting past messages**, and export or import — the day files are plain
  local files the owner can read.
- **Planning the day for the owner** — no time-blocking, no moving a task's time on its
  own, no focus timers; it suggests in words and the owner decides.
- **Priorities, tags, projects, sub-tasks** — a day's list is flat.
- **Mail** — Gmail or any other mail service, reading or replying: out of scope
  (decided 2026-10-02).
- **More than one persona**, more than one user, accounts, sync between Macs.

## 3. Features

### 3.1 Chat

#### Overview
- **Purpose:** the place where the plan goes in, where the secretary speaks, and where
  you answer it.
- **Access:** the main area of `bunshin tui`.

#### Specifications

| Item | Specification |
|---|---|
| Message input | one message at a time, at most 400 characters† |
| One message yields | zero or more changes (§3.3) and at most one reply |
| Reply length | at most 200 characters†, asked of the model in the operating rules (keeps the answer inside the model's window); a longer reply is shown whole, never cut |
| What the model sees | the instructions (§3.8), the app's operating rules, the current time, today's tasks, and as many recent messages as fit the window, oldest dropped first; today's open tasks are never dropped before chat history |
| Model window | 4,096 tokens shared by instructions, prompt, and answer; the prompt is sized to leave room for the answer |
| One call at a time | the model handles one request at a time; a message sent or a trigger fired during a call waits for it |
| Wait | a visible "thinking" state while the model runs; at most 30 s† before it gives up (§4) |

#### Edge Cases
- **Empty day:** the start of a day begins with the secretary asking for the plan (§3.6).
- **Long history:** the screen keeps every message of the day; only the model's view is
  trimmed.

### 3.2 Today's tasks

#### Overview
- **Purpose:** the list the secretary watches.
- **Access:** a task pane beside or above the chat (layout: [`ux-flows.md`](ux-flows.md)).

#### Specifications

| Item | Specification |
|---|---|
| Title | 1–80 characters† |
| Kind | **untimed**; **deadline** — due by a time (「15時までに資料」, shown as 〜15:00); **appointment** — happens at a time (「18時ジム」「10時から定例」, shown as 18:00) |
| Time | a local clock time HH:MM on the current day, for deadlines and appointments only |
| Status | open → done, or open → dropped (「今日はやめた」); either can be reopened. A task carried to the next day is marked carried over on its own day |
| Number | given in creation order, never reused within a day, shown in the list so a sentence or key can name it |
| Limit | 50 tasks a day† |
| Order | timed tasks by time, then untimed by creation; done and dropped below the open ones |
| Appointments | get no trigger of their own; the model sees them in every call and may bring one up |

#### Edge Cases
- **A time already past when added:** allowed (「10時の定例は出た」 at 14:00 records a done
  appointment); no trigger fires for it.
- **Limit reached:** adding the 51st task is refused with a message; nothing is dropped
  silently.

### 3.3 Task changes from plain words

#### Overview
- **Purpose:** 「15時までに資料作る」「会議終わった」 change the list without commands.

#### Specifications

| Item | Specification |
|---|---|
| Changes the model can propose | add (title, kind, time), mark done, drop, reopen, change time or kind, rename, mute for a while (§3.5) |
| Deadline or appointment | the model tells them apart (「までに」「締切」 → deadline; a time alone → appointment); the change line shows which, and a key fixes it |
| How a change is applied | at once, then shown in the chat as a change line listing each change |
| Ambiguity | when the model cannot tell which task is meant, nothing changes and the reply asks which one |
| Times | only an explicit clock time becomes a time (15時, 3時半, 15:30); 「午後」「夕方」 stay out of the time field |
| Validation | the core checks every proposed change (task exists, time valid, limits); an invalid one is dropped and the reply says so |
| Undo | see §3.4 |

### 3.4 Direct task editing by key

#### Overview
- **Purpose:** everything §3.3 can do is also one key away, so fixing a misread never
  needs the model.
- **Access:** the task pane has focus; the key table is in [`ux-flows.md`](ux-flows.md) §4.

#### Specifications

| Item | Specification |
|---|---|
| Operations | add, edit title, kind, and time, done / reopen, drop, delete, undo, mute / unmute |
| Delete | key only: removes a task added by mistake from the day; undoable |
| Undo | reverts the most recent change set — one message's changes, or one key action; pressed again it walks further back, up to 20† sets in this session; the undo is itself shown as a change line |
| Model involvement | none; every key works when the model is unavailable |

### 3.5 Proactive check-ins

#### Overview
- **Purpose:** the secretary speaks first.
- **Access:** automatic while `bunshin tui` is open.

#### Specifications

| Item | Specification |
|---|---|
| Tick | every 60 s† while the screen is open; a tick that finds no trigger calls no model |
| Trigger: before a deadline | a deadline task is still open 30 min† before its time |
| Trigger: after a deadline | a deadline task is still open when its time passes — once |
| Trigger: planned check | the time the model chose for its next look comes due; the model gives it as minutes from now, bounded 5–120 min† by the core |
| Trigger: day start / evening review / catch-up | §3.6 and §3.7 |
| No trigger for | appointments — there is no heads-up before one |
| Each trigger | fires at most once per task and kind; a trigger the model let pass is spent, and the model's planned check is how it comes back to it |
| Decision | when a trigger fires, the model returns {kind: silent, note, or question; the task it is about, or none; the message; minutes until its next look}; silent posts nothing; the model decides, the core enforces only the bounds in this table |
| What the model is told | the triggers, the instructions, today's tasks, yesterday's record (§3.6), recent chat, and the state of the last 5† unprompted messages — answered, acknowledged, ignored, and so on (§3.10) |
| Active hours | 08:00–22:00†: outside them no unprompted message is posted; triggers that fire outside are held and go to the model together as one message when active hours begin; held triggers of a previous day are dropped at the day start, whose leftovers cover them. The day start and the catch-up at an open answer the owner opening the screen, so neither active hours, the minimum gap, nor the mute holds them; the evening review and the catch-up after a sleep obey all three |
| Same task | at most one unprompted message about the same task in 30 min†; a message about a task inside that window is not posted and is recorded as suppressed |
| The list stays yours | a check-in never changes a task; only the owner's words (§3.3) and keys (§3.4) do |
| Minimum gap | 5 min† between any two unprompted messages; triggers that fire inside the gap wait for its end and go to the model together |
| Hourly cap | none |
| Mute | one key mutes unprompted messages for 60 min†, and the same key unmutes; 「1時間静かにして」 in chat proposes a mute of that length (5–480 min†) as a change; while muted, triggers are held and come as one catch-up message when the mute ends |
| Delivery | the message appears in the chat, marked as unprompted, labelled with its kind (note or question) and what triggered it (「締切30分前: 資料」), with a terminal bell; it also enters the inbox (§3.10) |
| While typing | an unprompted message is held until the input is sent or cleared |
| Conversation first | when an owner's message and a trigger are both waiting for the model, the message goes first |

#### Edge Cases
- **The model is unavailable:** deadline triggers are posted as fixed sentences
  (before: 「資料（〜15:00）の締切が近づいています」; after:
  「資料（〜15:00）の締切を過ぎました」); planned checks, the day start, and the review
  wait until the model is back.
- **The model call for a trigger fails:** a deadline trigger falls back to its fixed
  sentence; any other trigger is retried once at the next tick, then spent.
- **The model never plans a next look:** the core plans one at the upper bound
  (120 min†), so a day with no deadline is not silent forever.

### 3.6 The day's rhythm

#### Overview
- **Purpose:** a day starts with the plan and ends with a look back; leftovers are
  decided, not lost or silently piled up.

#### Specifications

| Item | Specification |
|---|---|
| Day boundary | 04:00† local time: from 00:00 to 03:59 it is still the previous day |
| Day start | at the first open of a day, or — if the screen stayed open across the boundary — at the first key press after it, so nothing rings at 04:00 |
| Leftovers | at the day start, the open deadline and untimed tasks of the last day on record are listed, and the secretary asks which to carry over and which to drop; the answer comes by chat (「全部持ち越し」「資料だけ持ち越し」) or by key; then it asks for today's plan |
| Appointments left open | are not offered as leftovers; they stay open on their own day |
| Carried-over task | becomes a new untimed task today with a new number; the original is marked carried over |
| Undecided leftovers | stay open on their day and are offered again at the next day start |
| Evening review | at 18:00†, or at the first open after 18:00† on the same day: a summary of what got done and what is open, and a question about the rest; nothing changes without the owner's word |
| Yesterday's record | at the day start the core writes a short record of the last day on record — how many tasks were done, carried over, dropped, and left open, with up to 3† titles of each — and every model call that day includes it (at most 120 tokens†); the core writes it, not the model |

#### Edge Cases
- **Several days closed:** the leftovers offered are those of the last day on record,
  however long ago.
- **Day start and review due at the same open:** the day start comes first; the review
  follows once it is settled.
- **Clock or time zone change:** times are local wall-clock times; a task's time is not
  shifted when the zone changes.

### 3.7 Persistence and catch-up

#### Specifications

| Item | Specification |
|---|---|
| What is kept | each day's tasks and chat, which triggers have fired, the secretary's planned next look, the mute |
| When | written on every change, and written whole or not at all, so a quit, a crash, or a reader such as `bunshin today` never sees half a file |
| Where | local files in the app's data directory, readable only by the owner (format and place: the `deciding-architecture` skill) |
| How long | every day is kept; nothing is deleted automatically; removing the files is the owner's step |
| One screen at a time | a second `bunshin tui` refuses to start while one is running, and says so |
| Catch-up | when the screen opens after being closed, or the tick resumes after a gap of more than 5 min† (the Mac slept), triggers that came due meanwhile go to the model together as **one** message, not a burst |

### 3.8 Instructions you write

#### Overview
- **Purpose:** the owner sets who the secretary is — its role, its tone, and what it
  should know about the owner (「平日は9〜18時、午前は集中したい」) — in one text.
- **Access:** a plain text file, opened in the owner's own editor with
  `bunshin instructions edit` (§3.9); one key in the TUI shows it read-only.

#### Specifications

| Item | Specification |
|---|---|
| Length | at most 600 characters† (about 300 tokens of the 4,096-token window) |
| Default | a short default ships with the app; `bunshin instructions edit` writes it into the file the first time, so the owner edits from it |
| What it cannot change | the app's own operating rules — the answer format, the current time, today's tasks — are added after the owner's text, every call |
| When a change takes effect | from the next model call; the screen can stay open |
| Viewing in the TUI | read-only: the instructions in use, their length against the limit, whether the default is in use and why, and the file's path; editing stays in the editor |

#### Edge Cases
- **Over the limit:** the model gets the default instead; the chat says so once, until
  the file changes; `bunshin instructions edit` reports it as soon as the editor closes.
  Nothing is cut silently.
- **Empty or missing file:** the default is used.

### 3.9 Subcommands

#### Overview
- **Purpose:** what a script, a status line, or the binary's own tests reach without the
  screen. Only `bunshin tui` writes the day's data, so no two writers ever meet.

#### Specifications

| Command | What it does |
|---|---|
| `bunshin tui` | opens the screen (§3.1–§3.8) |
| `bunshin today` | prints today's tasks — number, status, kind, time, title — to stdout; `--json` for scripts; an empty day prints nothing and exits 0 |
| `bunshin instructions` | prints the instructions in use (the file's, or the default) and the file's path |
| `bunshin instructions edit` | opens the file in `$VISUAL`, else `$EDITOR`; neither set → fails with the file's path and how to set one |

### 3.10 The inbox and reactions

#### Overview
- **Purpose:** what the secretary said or asked that the owner has not dealt with, in one
  list; and a record of how each unprompted message landed, which the next check-in
  reads.
- **Access:** the header shows how many items are open; one key opens the inbox (layout:
  [`ux-flows.md`](ux-flows.md) T4).

#### Specifications

| Item | Specification |
|---|---|
| What enters | every unprompted message (§3.5) |
| States | open → answered (a question got a reply), acknowledged (a note was acknowledged by key), dismissed (a question was set aside by key), task closed (its task was marked done or dropped), muted (the owner muted within 15 min† of it) |
| Ignored | a message still open 15 min† after it was posted is reported to the model as ignored; it stays in the inbox |
| What the inbox lists | today's open items, newest first |
| Answering | a message sent from the inbox answers the chosen question, and the input shows which; a message typed otherwise answers the most recent question asked in the last 15 min†, if there is one |
| Keys | answer, acknowledge, dismiss, go to its task; every one also works when the model is unavailable |
| Suppressed messages | (§3.5 "Same task") are recorded with the reason, never shown in the inbox |
| Day boundary | at the day start, the previous day's open items leave the inbox; the leftovers (§3.6) cover their tasks |

#### Edge Cases
- **Empty:** the header shows no count; the inbox says there is nothing to deal with.
- **The model is unavailable:** fixed-sentence deadline notes (§3.5) enter the inbox like
  any other note.

## 4. Cross-cutting rules

- **Privacy:** the model runs on the Mac; the app opens no network connection; tasks,
  chat, and instructions are kept only in local files readable by the owner alone; no
  log line carries chat text, a task title, or the instructions. The model is reached
  through one interface, so a hosted model could take its place later (Later) — this
  version uses only the on-device one.
- **Model unavailable** (`fm` missing, the model not downloaded yet, its terms not
  accepted, Linux): the screen says so once, naming the step for the owner to take — for
  the terms, running `sudo fm license` themselves; the app never runs it. Tasks and keys
  keep working; deadline triggers fall back to fixed sentences (§3.5).
- **Model failure on one call** (timeout, a refusal by the model's guardrails, output that
  fails the answer format): a user message yields no change and one short line, and can be
  sent again; a trigger is handled as §3.5 says.
- **Language:** the conversation is in Japanese, and so are the screen's own labels and
  the CLI's output ([`ux-guidelines.md`](../design/ux-guidelines.md), Language and copy).

## 5. Data

| Entity | Fields |
|---|---|
| Task | number (per day), title (≤ 80 chars), kind (untimed / deadline / appointment), time (HH:MM, deadlines and appointments), status (open / done / dropped / carried over), created at, closed at, triggers fired, origin (chat / key / carried over from a day) |
| Message | author (you / Bunshin / system), text, time, kind (reply / unprompted / change / notice / error) |
| Unprompted message | as Message, plus: note or question, its trigger, the task it is about, inbox state and when it changed, the question a reply answers, or suppressed with its reason |
| Day | logical date, tasks, messages, next planned look, time of the last unprompted message, muted until, held triggers, yesterday's record |
| Instructions | one text file, ≤ 600 characters |

Every file carries a format version, so a later version of the app can read an earlier
day. Nothing is deleted automatically; uninstalling the binary leaves the data where it
is, and the README says where that is.

## 6. Open questions

- None that change scope. Every † value is a starting value to tune in use; the data's
  place and format, and the instructions file's place, are settled in the
  `deciding-architecture` skill (2026-10-02).

## 7. Decision log

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
