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
| `docs/product/ux-flows.md` | the board screen, key table (§3), and flows F1–F3 | the requirements' §7 |
| `docs/design/ux-guidelines.md` | app-wide behavior: states, feedback, copy, accessibility | its "Decision log" table |
| `docs/design/design-direction.md` | the color roles (the lock is `deciding-architecture`'s) | — |
| `AGENTS.md` › Product | what the app is, its core interaction, its non-goals, in short | — |

Values marked † in the requirements are starting values in core's `Tuning`; changing one
is tuning, not a product decision, unless it changes what the owner sees happen.

## Feature map

**MVP** — the board prototype in requirements §2, with the screen and flows that show
the owner and characters sharing one post list:

| Feature | Requirements | Screens and flows |
|---|---|---|
| Character turns and topics | §3 R1–R4 | F1 |
| Owner posts and responses | §3 R5–R8 | F2 |
| Model call and failures | §3 R9–R11; §4 | F1, F3 |
| Board screen, scrolling, and input | §3 R12 | Screen, key table, F1–F3 |
| In-memory limit and command line | §3 R13–R14; §5 | Screen |

**Later** — none selected for this prototype.

**Non-goals** — §2 and `AGENTS.md` › Product must agree: network access; acting outside
the app; built-in voice; accounts, multiple users, or Mac-to-Mac sync; persistence,
history, export, or import; editable characters or topics; threads, mentions, emoji
reactions or likes as separate post controls; editing or deleting posts; notifications, bell, or sound.

## Intake

Run every step, in order, before an issue is filed for new or changed behavior.

1. **Restate the request** as what the owner would see happen, in one or two sentences,
   and find the feature in the map it extends. A request that fits nowhere is a new
   feature.
2. **Check the non-goals.** If the request crosses one, say which line, and stop unless
   the owner moves it. Moving a non-goal is the owner's call: it changes §2, `AGENTS.md`
   › Product, and this map together, with a §7 entry saying what moved and why. A Later
   item pulled forward is the same kind of decision.
3. **Scrutinize.** Look for what the request does not say: empty, failure, and
   model-unavailable states; how the posting interval, owner response turns, and scroll behavior interact; post and context limits; a contradiction with an existing value.
   Each finding becomes a question.
4. **Settle the values with the owner** in rounds of at most four questions, each with
   2–4 options, what each costs, and a recommendation when there is a real one. Name the
   requirement line each option serves or bends. Never fill a value the owner has not
   chosen; a starting value the owner accepts is marked †.
5. **Amend the documents** in place, so they describe the app after the change:
   requirements (§2, the feature's §3 table, §4, §5 as touched), `ux-flows.md` for a
   screen, a key, or a flow, `ux-guidelines.md` for an app-wide rule. Append the
   decision to the log that document uses, dated, with the rejected options and why.
   **REQUIRED:** if the change touches anything `deciding-architecture` lists — a port, a
   dependency, persistence, the model call, the color lock — that decision is
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
