# Detached task dispatch

Detached dispatch lets one BitFun process submit work to another BitFun process
without making the submitting process part of the execution topology. The
submitter can disconnect or exit after the target has durably acknowledged the
job.

This document defines the product boundary, workspace delivery contract, and
transport invariants. It complements
[`remote-workspace-transport.md`](remote-workspace-transport.md) and
[`peer-device-mode.md`](peer-device-mode.md); neither of those features is a
dispatch data plane.

## Ownership model

There are three roles:

- The **controller** selects a target, creates and claims a managed baseline
  worktree, submits a job, and observes it by cursor.
- The **target host** owns the job, worker process, local session, workspace
  lease, event log, permission mailbox, and terminal state.
- A **transport adapter** moves the same narrow JSON protocol over SSH or an
  end-to-end encrypted account-device RPC.

The controller is never a runtime or filesystem proxy for a non-local job. It
stores only an outbound observer record under
`~/.bitfun/dispatch/outbound/`; it must not create the target session in the
controller's normal session store. A target session is an ordinary local
session on the target and can be resumed there.

The Relay is an opaque router. Device requests, Git bundle chunks, and responses
are encrypted with the account master key before the Relay receives them.
Relay storage is not used for repository contents.

## Workspace delivery

Dispatch workspace delivery is Git-only. The composer exposes the dispatch
picker only for a Git repository or managed worktree, and non-UI callers are
validated again by the controller. A caller cannot nominate an unrelated
directory on the target as the execution root. Target discovery exposes an SSH
connection's identity and display metadata, but does not turn its saved
`defaultWorkspace` into a Dispatch execution directory; every detached job gets
the managed target worktree described below.

### Controller baseline

After the target protocol is known to be compatible, the controller uses
`WorktreeService::create` to create a normal managed worktree at the selected
base revision (`HEAD` by default). The revision is resolved once to an
immutable commit before delivery. The worktree is job-scoped and receives the branch
`<branchPrefix>/dispatch/<jobId>`, where `branchPrefix` comes from the shared
worktree settings.

The outbound record persists the baseline worktree id and path, base commit,
branch, source repository identity, optional remote URL, and last synchronized
head. The worktree is claimed by `dispatch:<jobId>`, which exempts it from
automatic worktree retention while the outbound job still depends on it. Claim
cleanup is ordered: first release `dispatch:<jobId>`, then delete the outbound
record. If release fails, the record is retained so cleanup can be retried and
the claim cannot be orphaned. The worktree remains a normal managed worktree:
users can inspect it with ordinary Git tools, and the normal worktree retention
policy resumes after release.

If `includeUncommitted` is selected, the normal worktree copy-local-changes
path is used and Git-visible changes are committed inside the baseline
worktree. The user's checkout is never staged, committed, switched, reset, or
otherwise changed. Git-ignored runtime inputs such as local `.env` files and
build output are not delivery inputs.

### Controller preparation journal

Setup that can outlive one request is recorded before the outbound submit is
acknowledged. The controller keeps an owner-only crash journal at
`~/.bitfun/dispatch/outbound/.preparations/<jobId>.json` and retains it through
the target's validated submit acknowledgement. Preparation, retry, and recovery
for the same job are serialized by one per-job run lock, so an expired-entry
recovery cannot race a live attempt.

For automatic SSH CLI setup, audit progress is a fallible durable write rather
than an in-memory callback. The `cli-install-started` transition is persisted
before the first remote installer mutation, subsequent success or failure is
persisted before setup returns, and any audit write failure stops submission.
Keeping the journal until acknowledgement lets a retry recover setup actions
that completed remotely even if the controller exited before submit finished.

Before creating the baseline claim, preparation records the stable project
workspace path that owns the worktree registry rather than relying on a linked
worktree path that may disappear. An expired preparation lease is necessary but
not sufficient for recovery to release `dispatch:<jobId>`: no durable outbound
record may own the same baseline. A matching outbound owner conservatively
keeps the claim, and an outbound-owner read error is treated as ambiguous and
also keeps it. Only an expired, provably unowned preparation may release its
claim; cleanup failures retain recoverable state for a later retry.

### Target repository and worktree

The controller resolves the repository remote URL when one exists and derives
a stable `repoKey`. The target keeps an owner-only bare repository cache at:

```text
~/.bitfun/dispatch/repos/<repoKey>
```

`workspace-provision` creates or refreshes that repository, fetches its remote
without interactive credential prompts, and checks for the requested base
commit. When the commit is reachable, the target creates the job worktree and
branch at:

```text
~/.bitfun/dispatch/worktrees/<jobId>
```

If the target cannot reach the commit, it returns `needsBundle` and the commits
it already has. The controller then creates a Git bundle advertised by the
job's named branch, excluding known target tips where possible. Bundle upload
is bound to the job, size, and SHA-256; the target verifies the digest, runs
`git bundle verify`, imports the branch into the bare repository, and retries
provisioning. This covers unpushed commits and repositories with no usable
remote without requiring origin write access.

SSH carries large bundle bytes over the established SFTP channel. Account-device
delivery uses bounded base64 chunks inside the end-to-end encrypted
`HostInvoke` envelope. Both transports use the same provision, digest,
idempotency, branch, and target-path contract. Repository caches that are no
longer referenced are eligible for retention cleanup after 30 days.

## Synchronization semantics

The controller baseline and target worktree start at the same immutable base
commit. They are not a live shared directory: edits made later in the user's
checkout are unrelated to the running job, and target writes remain on the
job branch until the user requests synchronization.

The one-click synchronization operation is available while a job is running
and after it reaches a terminal state:

1. The target stages Git-visible changes in its job worktree and creates a
   commit when necessary.
2. The target validates that the worktree is still on the job's named branch.
   For the first synchronization, `knownHead` is the immutable `baseCommit`;
   afterward it is the last head successfully stored by the controller. The
   target accepts `knownHead` only when it resolves to a commit and is an
   ancestor of the current branch head.
3. The target creates an incremental Git bundle for
   `<knownHead>..<branch>` and reports its branch, head commit, commit count,
   changed-file list, size, and SHA-256.
4. The controller transfers and verifies the bundle, checks that its managed
   baseline is still on the same job branch, fetches the reported branch into
   the baseline repository, and advances the baseline worktree with
   `--ff-only`.
5. The outbound record stores the synchronized head and the transfer artifact
   is removed only after that advance succeeds.

Each controller-side synchronization invocation carries a fresh `operationId`,
and every poll for that invocation reuses it. This keeps a completed no-op
(`headCommit == knownHead`) idempotent for its current poll loop while still
letting a later click at the same head start a new check for work produced by a
still-running agent. A changed result remains cached until its head is
acknowledged, so a failed bundle transfer or local fast-forward can retry safely.

The user's checkout is never changed by synchronization. The baseline
worktree is the review boundary; the user can test there and then merge or
rebase the dispatch branch with ordinary Git tools. There is no path-overwrite
or conflict-resolution mode. If either worktree left the named job branch, or
if the baseline was deleted or gained divergent commits, synchronization fails
visibly instead of resetting or rewriting branch metadata. A checkpoint while
the job is running can collide with a transient Git or index lock held by the
worker; that failure is retryable, does not advance `knownHead`, and the same
synchronization request can be retried after the lock clears.

## Protocol

The target CLI owns transport-independent dispatch protocol version 3 and the
durable store. Version 3 is intentionally incompatible with targets that do
not implement Git worktree delivery. SSH submission can repair that mismatch
through signed release installation; an account device must be upgraded as a
BitFun device.

Public job verbs are:

| Verb | Purpose |
| --- | --- |
| `probe` | Negotiate version/capabilities and inspect an optional target path. |
| `submit` | Durably create an idempotent job and detach its worker. |
| `status` | Read target state, event pages, completeness facts, and pending permissions. |
| `cancel` | Persist cancellation intent and stop the authenticated worker process group. |
| `list` | List durable target jobs. |
| `answer` | Resolve one persisted permission request for `remote` approval policy. |
| `append` | Queue an idempotent steering message for the active turn. |

Git delivery and synchronization use these internal data-plane verbs:

| Verb | Purpose |
| --- | --- |
| `workspace-provision` | Ensure the shared bare repository contains `baseCommit` and create the job branch and worktree, or return `needsBundle`. |
| `workspace-bundle-begin` | Bind an owner-only incoming bundle to its job, size, and SHA-256 and report the retained offset. |
| `workspace-bundle-chunk` | Append one bounded account-device bundle chunk at the expected offset. |
| `workspace-bundle-commit` | Verify and import the completed base bundle into the target repository. |
| `workspace-sync` | Commit target changes when needed and create the branch bundle returned to the controller. |
| `workspace-sync-chunk` | Read one bounded account-device result chunk. |

These verbs are not normal product commands. Account-device transport reserves
the corresponding `dispatch_target_workspace_*` names and routes them directly
to the target CLI before the attached Peer Host bridge.

Every compatible target advertises `workspace_git_worktree`,
`workspace_git_bundle_upload`, and `workspace_git_sync`. These are required
capabilities, not optional feature detection. `workspace_serialization`
continues to guarantee that workers sharing one canonical execution path are
locked correctly.

`dispatch_worker_cli_profile` is a required execution-safety capability. It
means every dispatch process selects `DeliveryProfile::Cli` before model/config
inspection can lazily initialize product-full tool state. Controllers must
check it both during target setup and immediately before submission; package
version equality is not evidence of this behavior.

`probe` is read-only and never installs software. Immediately before SSH
provisioning, submission probes again and automatically installs or upgrades a
compatible prebuilt `bitfun` release when needed. Release resolution stays
bound to the expected OS and architecture; the controller verifies the
checksum sidecar, its publisher signature when present, and the mandatory
archive signature, pins the SHA-256 passed to the installer, waits with a
bounded deadline, and probes the installed binary again before continuing.

A source build is different: it uploads the controller's repository and
compiles it on the target. It therefore remains a separate user-confirmed
operation and uses only the clean, confirmed controller revision. Automatic
prebuilt installation never escalates to a source build.

Account-device transport wraps target verbs in names reserved for detached
dispatch, such as `dispatch_target_submit`. They are handled before the
attach-shaped Peer Host bridge and never acquire an attached-controller lease.
Conversely, controller-side commands such as `dispatch_submit` remain
local-only in every Peer Device Mode deny table. Disconnecting the last Peer
controller must not cancel or hide a detached dispatch job.

An account target must already have a compatible `bitfun dispatch` runner. A
CLI daemon already satisfies this. The Desktop account host delegates to an
installed `bitfun` binary (including a package-manager symlink); if none is
available, probe reports the missing runner and submission remains disabled
rather than falling back to local execution. Device dispatch never performs
SSH-style installation through the Relay.

## Event and observer contract

The target event log is append-only within a retained window. A status response
reports:

- the next byte cursor;
- whether the requested cursor was reset;
- whether older history was truncated;
- how many oversized events were replaced by visible omission markers;
- whether the returned transcript can be considered complete.

Rotation and oversized events must never be represented as a complete
transcript. Multiple controllers may observe the same job because cursors are
per observer and the target has no controller lease. Explicitly listing jobs
for a selected target adopts observer-only routing records on that controller;
it does not copy sessions or acquire workspace/runtime ownership.

A cursor records how far into the event log an observer has read, not what it
drew, so a cursor on its own cannot restore a projection. The controller
therefore caches each observer's rendered transcript beside its outbound index,
one file per job, holding the projected turns together with the cursor that
produced them and the completeness facts that applied at that point. Storing
them in one document is what keeps them consistent: a restart resumes from the
cached cursor rather than any other stored one, because only that pair was
written together, and a truncated history stays marked as truncated. The cache
is versioned by the projection rules that wrote it, and anything missing,
corrupt, mismatched, or above the size ceiling replays the job from byte zero.
The controller stores the projection verbatim and never interprets it; caching
it creates no durable session and acquires no runtime ownership.

Target and outbound records are retained for 30 days after terminal state, as
are the cached transcripts, which are also dropped as soon as a projection is
deleted or archived. Garbage collection never removes queued or running jobs.
Target cleanup is limited to the job's managed worktree and private transfer
state; shared repository caches have their own last-used retention check. On
the controller, expiring an outbound record releases its baseline worktree
claim before normal worktree retention can consider that worktree.

## Approval and supervision

Every submit requires an explicit policy:

- `auto` uses the shared Runtime auto-approval metadata.
- `reject-and-report` fails closed when confirmation is required.
- `remote` disables inherited auto-approval while keeping user input
  available. The worker persists the safe presentation DTO, status exposes it,
  and `answer` records a user-sourced reply before execution resumes.

Permission responses and appended messages are target-owned mailboxes. They are
idempotent across controller retries and do not depend on the controller that
originally submitted the job.

## Failure rules

- A missing or offline target fails submit; the Relay does not queue jobs.
- A target missing a required behavioral capability fails preflight before a
  durable job is created, even when its CLI package version matches.
- A signed prebuilt SSH release may be installed automatically. Missing
  platform support, failed signature or digest verification, installer timeout,
  or an incompatible post-install probe fails closed before workspace
  provisioning.
- A lost submit response leaves `submission_unknown`; status or an idempotent
  retry reconciles the target's durable truth.
- A live PID that no longer matches the exact worker command is never signaled
  and settles the job failed instead of leaving an orphaned non-terminal job.
- A target restart after the Runtime accepted a turn does not replay the prompt,
  because replay could duplicate tool side effects. The native target session
  remains available for manual resume.
- Prompt and event pages remain below the smallest host transport envelope.
- Invalid repository keys, commit ids, branch names, bundle paths, offsets,
  sizes, digests, prerequisites, or Git verification results fail before job
  submission. No failure may redirect a transfer outside the managed dispatch
  directories or silently run against an unrelated target directory.
- A missing baseline worktree or rejected fast-forward leaves both Git histories
  intact and returns a visible synchronization error.
