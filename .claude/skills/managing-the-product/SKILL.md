---
name: managing-the-product
description: >
  Holds Bunshin's feature map (the MVP features, the Later list, and the non-goals, each
  with its section in docs/product/requirements.md) and the intake every feature request
  or scope question goes through before an issue is filed: checking it against AGENTS.md's
  Product section and the non-goals, settling its values with the owner in option rounds,
  amending requirements.md, ux-flows.md, or ux-guidelines.md with a decision-log line, and
  handing the work to triaging-issues. Use when the owner asks for a new feature or a
  change in behavior, asks whether something is in scope, reports a friction from daily
  use that implies new behavior, before filing an enhancement or tracking issue, or when
  a requirement or a non-goal moves.
---

# Managing the Product

**Owns:** the feature map and the intake of a feature request before any issue is filed.
**Does not own:** an issue's labels and body (`triaging-issues`); the order of outcomes
(`steering-the-roadmap`); architecture decisions (`deciding-architecture`); a bug, which
is a gap between the code and a requirement that already exists and goes straight to
`triaging-issues`.

The requirements are the product's memory. A feature that reaches the tracker without
passing them leaves `docs/product/requirements.md` describing an app that no longer
exists, and the next agent implements against the stale text. This skill keeps the
documents ahead of the issues: the document changes first, and the issue cites it.

## The documents

| Document | Holds | Its decision log |
|---|---|---|
| `docs/product/requirements.md` | scope (§2: MVP, Later, Non-goals), each feature's values (§3), cross-cutting rules (§4), data (§5) | §7 |
| `docs/product/ux-flows.md` | screens T1–T5 and C1–C4, the key table (§4), flows F1–F10 | the requirements' §7 |
| `docs/design/ux-guidelines.md` | app-wide behavior: states, feedback, copy, accessibility | its "Decision log" table |
| `docs/design/design-direction.md` | the color roles (the lock is `deciding-architecture`'s) | — |
| `AGENTS.md` › Product | what the app is, its core interaction, its non-goals, in short | — |

Values marked † in the requirements are starting values in core's `Tuning`; changing one
is tuning, not a product decision, unless it changes what the owner sees happen.

## Feature map

**MVP** — what the first version is (requirements §2), with the screens and flows that
show it:

| Feature | Requirements | Screens and flows |
|---|---|---|
| Chat | §3.1 | T1 |
| Today's tasks | §3.2 | T1, T2, C1 |
| Task changes from plain words | §3.3 | T1, F2 |
| Direct task editing by key | §3.4 | T1, T2, key table, F3 |
| Proactive check-ins | §3.5 | T1, F4, F6 |
| The day's rhythm | §3.6 | T1, F1, F7 |
| Persistence and catch-up | §3.7 | C4, F8 |
| Instructions you write | §3.8 | T5, C2, C3, F9 |
| Read-only subcommands | §3.9 | C1–C3 |
| The inbox and reactions | §3.10 | T4, F5 |
| Model unavailable, failures, language | §4 | F10 |

**Later** — intended, not ordered (§2 "Later"; ordered ones are on the roadmap): macOS
notifications; a background process; multi-day tasks; looking back over past days;
writing from the shell; settings for the starting values; RSS digests; Obsidian vault
tasks; a hosted model behind the same interface; Calendar as a source.

**Non-goals** — §2 "Non-goals" and `AGENTS.md` › Product, which must agree: network
access in this version; acting outside the app; a general chatbot; built-in voice;
editing past messages, export, import; planning the day for the owner; priorities, tags,
projects, sub-tasks; mail; more than one persona, user, or Mac.

When the map changes, this table, §2, and `AGENTS.md` › Product change in the same
commit.

## Intake

Run every step, in order, before an issue is filed for new or changed behavior.

1. **Restate the request** as what the owner would see happen, in one or two sentences,
   and find the feature in the map it extends. A request that fits nowhere is a new
   feature.
2. **Check the non-goals.** If the request crosses one, say which line, and stop unless
   the owner moves it. Moving a non-goal is the owner's call: it changes §2, `AGENTS.md`
   › Product, and this map together, with a §7 entry saying what moved and why. A Later
   item pulled forward is the same kind of decision.
3. **Scrutinize.** Look for what the request does not say: the empty, error, and
   model-unavailable states; what happens at the 04:00 boundary and on reopen; the key
   that does the same thing (§3.4: everything the model can do is one key away); the
   one-writer rule (§3.9); the token budget (§3.1); a contradiction with an existing
   value. Each finding becomes a question.
4. **Settle the values with the owner** in rounds of at most four questions, each with
   2–4 options, what each costs, and a recommendation when there is a real one. Name the
   requirement line each option serves or bends. Never fill a value the owner has not
   chosen; a starting value the owner accepts is marked †.
5. **Amend the documents** in place, so they describe the app after the change:
   requirements (§2, the feature's §3 table, §4, §5 as touched), `ux-flows.md` for a
   screen, a key, or a flow, `ux-guidelines.md` for an app-wide rule. Append the
   decision to the log that document uses, dated, with the rejected options and why.
   **REQUIRED:** if the change touches anything `deciding-architecture` lists — a port, a
   dependency, the day file's format, the model call, the color lock — that decision is
   proposed there too, and the owner accepts it before issues are filed.
6. **Hand off.** **REQUIRED:** `triaging-issues` for each issue's type, tier, and body;
   every body cites the requirement section it implements rather than restating it. A
   feature that needs more than one issue gets a tracking parent with sub-issues. If the
   feature becomes an outcome the owner wants ordered, propose its line through
   `steering-the-roadmap`.

The document change and the issues land in that order: the amended documents are
committed (or in the pull request the issues will cite) before an issue names them.

## Answering "is this in scope?"

Answer from the map, with the line that decides it: in the MVP (which §), in Later (and
what would bring it forward), a non-goal (which one), or not covered — then offer the
intake. Do not answer from memory of a conversation; the documents are the record.

## Frictions from daily use

A friction the owner reports while using the app is one of three things:

- the code does not do what a requirement says — a bug (`triaging-issues`);
- a † value feels wrong — tuning; propose the new value, and record it in §7 only if
  the owner's visible behavior changes;
- the requirements do not cover it — a feature request; run the intake.

Say which one it is before acting. An agent never files an issue for a friction it
noticed on its own without the owner's yes (`AGENTS.md` › "Security and human
approval").

## Checks

`mise exec -- typos` on every changed Markdown file; `just check-harness` (the Product
section, skill references, no issue or pull-request references in a standing document).
