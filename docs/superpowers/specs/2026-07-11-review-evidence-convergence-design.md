# Review Evidence Convergence Design

## Context

PR #1502 adds prepared target evidence for Review and Deep Review, but the
current implementation mixes correctness controls with speculative caching,
prompt-heavy execution metadata, and advisory limits that are not enforced by
the runtime. It also leaves several fail-open paths: mutable workspace evidence
can still support a clean approval, malformed reports default to approval,
explicit root files can expand to the workspace, and an uncertain launch can
delete a session that the backend already accepted.

This revision keeps the useful target-binding work while removing or deferring
the parts that do not improve correctness in the current product.

## Goals

- A Review result must never look clean when its evidence is missing, partial,
  stale, or failed.
- Explicit targets must resolve exactly; ambiguity must stop before launch and
  must never broaden to the workspace.
- Diff paging must be bounded and sequential without exposing arbitrary byte
  offsets to reviewers.
- Old Review sessions must remain loadable after the contract changes.
- Ordinary Agent prompts, tool schemas, short results, and execution behavior
  must remain unchanged.
- The implementation must reduce prompt metadata and avoid adding a new
  persistent ledger, snapshot service, or provider workflow.

## Non-goals

- Provider-backed pull-request discovery, commenting, or inline comments.
- A reusable immutable workspace snapshot or incremental-review cache.
- A persistent cross-process token or evidence budget ledger.
- Replacing the existing Deep Review work-packet, roster, queue, or judge
  contract.
- Adding a new model recommendation enum solely to represent evidence quality.

## Contract decisions

### 1. Evidence state is separate from the model recommendation

The deterministic runtime owns an evidence state:

- `complete`: the bound evidence can support the normal recommendation.
- `limited`: the run intentionally lacks complete immutable evidence.
- `stale`: the workspace or target binding changed during the run.
- `failed`: required evidence or a valid final summary is unavailable.

The model continues to use the existing recommendation values. Publication,
export, and remediation consume the effective result: only `complete` evidence
may preserve a clean approval; every other state is deterministically converted
to a non-clean outcome with an explicit reliability notice. The conversion is
performed on the report rather than returned as a tool error, preventing model
retry loops and additional token use.

For the current PR, explicit Git ranges can become `complete`. Workspace reviews
remain `limited` because the prepared fingerprint is not an immutable content
snapshot. `stale` is reserved for a binding change observed by the existing
runtime/session facts; this PR does not add a final whole-worktree rescan or a
snapshot service merely to manufacture that distinction.

### 2. Preserve old-session compatibility

`review_target_evidence` remains readable as a legacy persisted field. New
launches use the current manifest/evidence location, while restore code converts
the legacy field into the runtime view when the new location is absent. The
field is not deleted until persisted-session compatibility has a separate,
versioned migration.

Deep Review keeps its strict manifest extension (`workPackets`, reviewer roster,
queue policy, quality gate, and strategy decision). Standard Review shares only
the stable target-evidence facts; it does not acquire the full strict manifest.

### 3. Bound diff reads with a server cursor

Review-only `GetFileDiff` requests use an opaque, backend-issued continuation
cursor. The first call has no cursor. A continuation must match the same
session, evidence binding, file, and previous page. Arbitrary offsets are not
accepted by the Review schema.

The existing session runtime stores only lightweight page state:

- the next cursor for each bound file;
- pages already returned, so an identical retry is served without another Git
  read or another charge;
- cumulative returned characters and calls for the current Review session.

When restored state cannot prove the remaining allowance, the evidence becomes
`limited` rather than constructing a new persistent ledger. Budget exhaustion
returns a structured limited page/result, not a tool failure. The budget covers
model-visible diff evidence returned by this tool; the PR does not claim to
measure all provider tokens.

### 4. Exact scope resolution

The command parser and resolver recognize root-level files and quoted or
space-containing paths. They preserve deleted and renamed targets and distinguish
files from directories. A missing, outside-workspace, nested-repository,
symlink/reparse, or otherwise ambiguous explicit target produces a localized
pre-launch error or preview; it never falls back to whole-workspace Review.

### 5. Idempotent launch recovery

Every launch carries a request id. If `sendMessage` returns an uncertain error,
the frontend queries backend/session state by request id before cleanup. It
deletes only a session proven not to have accepted the request. Accepted or
still-running requests remain attached to their session.

## Over-design rollback

Remove metadata that has no current runtime consumer or repeats facts already
available in the typed manifest:

- synthetic diff references that are not immutable references;
- the speculative incremental-review cache plan;
- repeated evidence/strategy/token prose in launch prompts when the backend
  already injects the typed manifest;
- advisory "two page" or prompt-byte claims that the runtime does not enforce.

Keep cost estimation and user consent because they are product controls. Keep
Deep Review work packets and quality-gate fields because the current runtime
uses them; refactoring those fields requires separate measurements and behavior
equivalence work.

## Data flow

1. The frontend parses an exact target and requests target evidence.
2. The backend validates the evidence and persists the compatible session view.
3. Review-only diff reads consume the bound file list through backend cursors.
4. Runtime evidence state can degrade from `complete` to `limited`, `stale`, or
   `failed`; it never upgrades again within the same run.
5. The final report is parsed once. One deterministic structured repair is
   allowed for a malformed/missing summary; a second failure marks the report
   `failed` without defaulting to approval.
6. UI, export, and remediation use the effective evidence-aware result.

## Error handling

- Expected launch and target failures use localized error codes, not raw backend
  English strings.
- Cursor mismatch, exhausted allowance, and unavailable restored allowance are
  structured evidence limitations rather than retriable tool errors.
- A stale workspace preserves findings for historical value, blocks a clean
  result, and offers a fresh rerun.
- Missing or invalid final summaries never synthesize low risk or approval.

## Verification

- Rust tests for ordinary Agent schema/result isolation, legacy session restore,
  evidence-state monotonicity, effective recommendation, cursor sequencing,
  retry caching, and exhausted/restored allowance behavior.
- Web tests for root files, spaces, deleted files, renames, directories,
  ambiguous targets, localized errors, evidence-aware report actions, and
  request-id launch recovery.
- Existing focused GetFileDiff tests must pass on Windows, Linux, and macOS
  semantics, including symlink and nested-repository cases.
- `pnpm run type-check:web`, focused Web tests, focused Rust tests,
  `cargo check --workspace`, `cargo check -p bitfun-desktop`, Rust formatting,
  i18n audit when resources change, and `git diff --check`.

## Follow-up gate

Provider Review remains the only planned follow-up and stays limited to opening
the existing Review flow from provider facts. Immutable workspace snapshots,
incremental caches, adaptive reviewer planning, or a persistent budget service
require production evidence first. At minimum, future proposals must show no
ordinary-Agent prompt/schema growth, zero stale clean approvals, exact-scope
success, no orphan launches, non-increasing Review token P50/P95, and no more
than a two-percentage-point recall regression on the agreed benchmark.
