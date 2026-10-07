# Initial review, current-head CI and landing

## Record reviewed and current revisions separately

Record repository, issue, PR URL/number, initial review ID and commit, current head
SHA, base SHA/ref, correction commits and check/run evidence. Refresh head/base when
checking CI or landing. Never present the initial review as cloud review of later
corrections. A changed diff invalidates old CI and local acceptance evidence, but
completion of the initial review remains recorded for this PR.

The owner chooses one cloud review per PR: wait for the automatic opening review,
address its findings, then use local verification and current-head CI for landing.
Do not request or wait for a second cloud review after corrections or base integration.
This accepts the risk that later changes have no fresh cloud review. It does not
remove repository-required approvals, checks or unresolved correctness decisions.

## Wait for the first automatic review

Discover GitHub MCP actions and inspect review submissions, inline threads, the PR
conversation and review-task metadata. Follow pagination or another MCP action when
normalized responses omit data. Report required inaccessible evidence; never use
`gh` or direct HTTP as a fallback.

Open a regular PR. Record the opening event and the automatic review's trusted
integration identity, state and reviewed commit. Wait through its queue/start delay
and running state. Do not post `@codex review`, `@codex fix`, change review settings,
or close/reopen the PR to start another run. Pushes do not imply a new automatic run.
If the initial run fails or cannot be observed, report that blocker without claiming
completion or replacing it with a local review.

| Initial review state | Action |
| --- | --- |
| Not started / queued | Wait for the opening-triggered review. |
| Running | Observe review and CI in bounded waits. |
| Completed, findings | Read all findings and address accepted in-scope ones. |
| Completed, no findings | Record terminal state and the reviewed commit. |
| Failed / unavailable | Report missing evidence and preserve the PR. |

Completion must be terminal and come from the trusted Codex integration, associated
with this PR and its reviewed commit. A display name, acknowledgement, reaction,
green CI or silence alone is insufficient. A trusted terminal summary with matching
commit metadata, a no-findings signal and empty findings/threads can establish a
clean initial review. Completion alone does not mean there were no findings.

Classify each finding as accepted, rejected or out of scope with a reason. Fix
accepted in-scope findings, inspect the complete correction diff, run affected checks
and `just check`, commit with normal hooks and push. Record which commits address
which findings. An unresolved owner decision or correctness finding blocks landing;
CI success cannot dismiss it. Resolve threads only within comment/thread-write
scope, and do not require a fresh cloud review to prove the fixes.

If an unsolicited later review appears, do not wait for its completion. Address any
known actionable correctness feedback already received; do not ignore a known defect
merely because the initial review is complete.

## Verify CI on the current head

Inspect workflows, jobs/steps and required checks for the latest head through MCP,
including pagination. Compare with the committed ruleset and workflows; inspect
live requirements/approvals where MCP exposes them. Report unavailable policy fields
without inventing approvals or bypassing server-enforced requirements.

A green check for one job, an earlier SHA or a partial response is insufficient.
Distinguish success, failure, pending, skipped, cancelled and inaccessible checks.
A skipped required check is not success unless active policy explicitly permits it.
Diagnose failures while the initial review is pending. Any correction or changed
base integration requires fresh local acceptance and CI for the resulting head.
Retries must be authorized and associated with their actual head/run. Never change
settings or dispatch workflows merely to obtain missing evidence.

Use bounded observations (at most 60 seconds per blocking wait) and back off when
state is unchanged. Continue in-scope repairs without an arbitrary attempt cap.
Pending gates or elapsed time alone do not complete or fail a run. Honor user
deadlines and host limits. Once the initial review is complete, accepted findings
are addressed and current-head gates pass, no further review wait is required.

## Land within recorded authorization

At a PR-only boundary, preserve the open PR. For an authorized merge:

1. Refresh head/base, mergeability, known findings, required approvals and all
   expected checks. Confirm the initial review completed, accepted findings have
   correction evidence, and local acceptance/CI cover the current diff. Integrate
   the current default branch normally if needed, repeat local checks and CI, and
   inspect the resulting diff. Do not request another review or force-push.
2. Merge through MCP with the expected-head-SHA guard and an allowed merge method.
   Never use an admin bypass. If the head changes or merge is rejected, inspect
   the actual state before retrying; do not route around a policy rejection.
3. Fetch the PR again and verify merged state, target branch and merge commit.
   Confirm the linked issue closed. If closure failed, close it only within
   explicit issue-closure scope; otherwise report the remaining open issue.
   A successful write response alone is not postcondition verification.
4. Preserve the primary checkout and evidence. Perform only authorized cleanup
   through the matching local or managed-worktree lifecycle and verify outcomes.

Report the initial reviewed commit, addressed findings, final head and CI separately,
including that corrections were locally verified without a second cloud review.
