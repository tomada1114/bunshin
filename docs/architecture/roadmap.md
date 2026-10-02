# Roadmap

This page records the app's direction: the outcomes it is working toward now, the ones
that come next, and the ones only intended for later. It sits between two other homes
and repeats neither:

- `AGENTS.md`'s `## Product` says what the app is, its core interaction, and its
  non-goals. Nothing here contradicts a non-goal; moving one is the owner's call, made
  in that section first (`managing-the-product`).
- The issue tracker holds the units of work, their priority labels, and their `blocked:`
  and `on hold` labels (`triaging-issues`). This page links issues and never copies
  their bodies.

It records direction and authorizes nothing. An issue is implemented because it is
filed, prioritized, and picked, never because a line here names it. It is not a
decision record either: a decision a line depends on lives in the `deciding-architecture`
skill and is named from here. What has shipped is in `CHANGELOG.md`, not on this page.

The owner decides what the page says; an agent proposes a change to it in a pull
request, and the change lands only once the owner has approved it.

- **Last reviewed:** 2026-10-02

## Now

The outcomes being worked on, one to three of them. Each has its issues filed.

- **Keep today's list by hand** — the list, its keys, and its files are what every
  later feature writes into, and they work with no model at all. Issues: [links].
  Done when: in `bunshin tui` the owner adds, edits, completes, drops, deletes, and
  undoes tasks by key (requirements §3.2, §3.4); after quitting and reopening the list
  is the same; `bunshin today` and `bunshin today --json` print it (§3.9); a second
  `bunshin tui` refuses to start (§3.7); and the template's counter is gone.
- **Tell it the day in words** — the core interaction's first half. Issues: [links].
  Done when: a sentence such as 「15時までに資料」 adds a deadline task, 「会議終わった」
  marks the meeting done, each shown as a change line and undone by one key (§3.3); the
  owner's instructions shape the reply and `bunshin instructions` prints them (§3.8);
  and with `fm` unavailable the screen says so and every key still works (§4).
- **Be nudged at the right moment** — the second half: it speaks first. Issues: [links].
  Done when: over one working day with the screen open, the day starts with yesterday's
  leftovers and the plan (§3.6), every open deadline task is considered before and after
  its time, check-ins come at intervals the model chose and never outside active hours
  or twice about one task in 30 minutes (§3.5), questions wait in the inbox until
  answered or dismissed (§3.10), the evening review comes at 18:00, a sleep or a reopen
  becomes one catch-up message (§3.7), and one key mutes it.

## Next

The outcomes that follow once Now's are done. An issue may already exist for one, often
parked as `on hold`; none is required.

- **Tune the starting values on real days** — every † value in the requirements is a
  guess until a few weeks of use. Before it moves up: the three Now outcomes in daily
  use, and the open items in `deciding-architecture`'s design (the `fm` error codes, the
  prompt language, greedy sampling) observed with `just test-local`.
- **macOS notifications when the terminal is behind other windows** — pulled forward if
  the bell is missed in practice. Before it moves up: the owner reports missing a
  check-in; a recorded decision on the notification mechanism and its permission.

## Later

Direction the app intends to take but has not ordered. No issue is filed for a line
here, apart from a parked one that a line names.

- **Writing from the shell** (`bunshin add`, `bunshin done`) — brought forward when the
  owner wants to add tasks without the screen; needs a decision on a second writer.
- **A background process** that keeps watching with the screen closed — brought forward
  if the screen being closed costs missed deadlines.
- **Multi-day and recurring tasks**, and **looking back over past days** — every day is
  already kept; brought forward when one day's list stops being enough.
- **Settings for the starting values** — when one value needs changing more often than
  a rebuild.
- **Outside sources**: RSS digests, Obsidian vault tasks, macOS Calendar, a hosted model
  behind the same interface — each needs its own decision on network access, a
  non-goal of this version.
