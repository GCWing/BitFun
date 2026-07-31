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

- The **controller** selects a target, prepares an optional workspace snapshot,
  submits a job, and observes it by cursor.
- The **target host** owns the job, worker process, local session, workspace
  lease, event log, permission mailbox, and terminal state.
- A **transport adapter** moves the same narrow JSON protocol over SSH or an
  end-to-end encrypted account-device RPC.

The controller is never a runtime or filesystem proxy for a non-local job. It
stores only an outbound observer record under
`~/.bitfun/dispatch/outbound/`; it must not create the target session in the
controller's normal session store. A target session is an ordinary local
session on the target and can be resumed there.

The Relay is an opaque router. Device requests, workspace chunks, and responses
are encrypted with the account master key before the Relay receives them. Relay
storage is not used for workspace contents.

## Workspace delivery

`workspacePath` in a submit request identifies a directory on the target. It
does not imply that similarly named directories on two machines are related.
Dispatch therefore supports three explicit delivery modes.

### Existing target directory

`existing` uses a directory that already exists on the target. Probe returns its
canonical path and Git facts before submit. BitFun never clones, fetches,
checks out, stashes, or rewrites that directory as part of dispatch.

### One-shot source snapshot

`snapshot-source` captures the controller workspace while honoring repository
ignore rules. It includes tracked and non-ignored source files, including
hidden source such as `.github/`, while excluding ignored dependency caches,
build output, and local secrets. It uses the same verified, one-shot upload,
materialization, result, and conflict rules as an exact snapshot. The filtered
input set is carried in the existing exact-snapshot wire envelope, so compatible
targets do not need a second materialization protocol.

This is the default snapshot choice for ordinary source workspaces. Users who
need ignored runtime inputs must choose the exact mode explicitly and confirm
its wider data boundary.

### One-shot exact snapshot

`snapshot-exact` captures the controller workspace at submit time and materializes it
below:

```text
~/.bitfun/dispatch/workspaces/<jobId>/current/
```

The snapshot includes regular files and empty directories, including hidden and
ignored files, because the mode promises the workspace's current contents
rather than a Git checkout. The controller must show that this can include
`.env`, local credentials, build output, and other ignored data and require
explicit confirmation.

The following entries are not silently copied:

- every entry named `.git`, because worktree pointers, nested object stores,
  hooks, and credentials are repository metadata rather than workspace input;
- symbolic links, to avoid following data outside the selected root or creating
  target-dependent aliases;
- sockets, devices, FIFOs, and other special files;
- paths that cannot be represented as portable UTF-8 relative paths.

Encountering any unsupported entry fails packaging and names the entry. A
successful manifest therefore describes every delivered entry; there is no
best-effort omission.

The archive and manifest are bounded by explicit file-count, per-file, and total
byte limits: 100,000 files, 100,000 directories, 256 MiB per file, 2 GiB
uncompressed, and 1 GiB compressed. The controller computes a SHA-256 digest
and sends immutable upload metadata. The target writes to an owner-only staging
file, rejects offset mismatches, verifies size and digest, validates every
archive path and entry type, extracts into a new staging directory, and
atomically publishes `current`. Materialization runs as a detached target
process; `workspace-commit` starts or polls it, so a single SSH or Relay RPC
timeout cannot kill a large extraction. A repeated begin/chunk/commit for the
same job and digest is idempotent; a different digest for the same job is a
conflict.

Packaging is one deterministic traversal, not an operating-system filesystem
snapshot. A file that changes size or modification time while it is read makes
the package fail, but coordinated edits across multiple files can still span
the traversal interval. Callers that require an application-consistent source
must quiesce the source or select a filesystem snapshot as the source path.

The controller retains the latest verified archive for each canonical source
path and capture mode. Before packaging a later job, it recomputes a lightweight
fingerprint from the selected paths and their filesystem identity, size,
executable state, and write/change timestamps. An unchanged fingerprint
hard-links the cached immutable archive into the new job instead of rereading
and recompressing every file.

That fingerprint is metadata-only, so operations that leave every byte intact
still change it: `chmod`, an editor's write-then-rename, a `git checkout` round
trip. Because the target's own cache is keyed by the archive digest, a
controller miss forces a full retransfer as well, so a changed fingerprint alone
is not allowed to condemn the cache. Packaging therefore publishes the archive's
per-file manifest as a sidecar next to the cached archive, and a fingerprint
mismatch falls through to comparing the source against it: first structurally,
by path, kind, size, and executable bit, which needs no more I/O than the
fingerprint itself and rejects nearly every real change; then, only when the
structure is identical, by per-file SHA-256. An identical tree reuses the cached
archive and writes the new fingerprint back, so the content comparison is paid
once rather than on every later job. A cache entry with no manifest sidecar —
one written by an older build — silently repackages as before.

Source mode ignores changes below ignored paths; exact mode observes them. A
selected entry change invalidates and atomically replaces the cache. The per-job
link remains immutable during submission, so a later cache replacement cannot
change an in-flight job's bytes.

SSH transports the archive with SFTP after `workspace-begin`. Account-device
RPC uses bounded base64 chunks inside the existing end-to-end encrypted
`HostInvoke` envelope. Neither transport puts source bytes in command-line
arguments, process listings, logs, or the outbound observer record.

After a target has fully verified and materialized a snapshot, it retains one
owner-only archive keyed by the archive SHA-256. A later job with identical
metadata attaches that immutable archive and reports the full retained offset,
so both SSH and account-device controllers skip the source transfer. The
temporary per-job archive link is removed after materialization; each job still
gets its own writable `current/` directory, so cache reuse never makes jobs
share writes. Cache entries expire after 30 days without a hit.

## Synchronization semantics

A snapshot is an immutable input boundary, not a live shared folder:

1. The controller captures version `S0`.
2. The target verifies and publishes `S0`.
3. The target becomes authoritative for all writes during the job.
4. Observers pull target events, permissions, and terminal state by cursor.

The controller does not mirror local edits made after `S0`, and target writes
are not merged automatically into a possibly changed controller workspace.
Continuous bidirectional synchronization would require conflict detection,
delete semantics, editor coordination, and a controller that remains online,
which contradicts detached execution.

Returning code is a separate, explicit result operation. A future result bundle
may expose an artifact or patch derived from `S0` and the terminal target tree;
applying it must remain a user-confirmed local operation. Until that operation
exists, the UI states that snapshot results remain on the target and shows the
managed target path.

## Protocol

The target CLI owns the transport-independent protocol and durable store.
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

Workspace upload uses the internal `workspace-begin`, `workspace-chunk`, and
`workspace-commit` verbs. They are target data-plane operations and are not
normal product or Peer Device Mode commands.

`dispatch_worker_cli_profile` is a required execution-safety capability. It
means every dispatch process selects `DeliveryProfile::Cli` before model/config
inspection can lazily initialize product-full tool state. Controllers must
check it both during target setup and immediately before submission; package
version equality is not evidence of this behavior.

CLI installation smoke-tests the same capability before replacing an existing
target binary. An untagged Desktop development build may, after the normal
explicit source-build confirmation, archive its clean current Git commit and
build that exact source on the target. This avoids reinstalling an older
same-semver release while keeping executable transfer an explicit user action.

Account-device transport wraps target verbs in names reserved for detached
dispatch, such as `dispatch_target_submit`. They are handled before the
attach-shaped Peer Host bridge and never acquire an attached-controller lease.
Conversely, controller-side commands such as `dispatch_submit` remain
local-only in every Peer Device Mode deny table. Disconnecting the last Peer
controller must not cancel or hide a detached dispatch job.

An account target must have a compatible `bitfun dispatch` runner. A CLI daemon
already satisfies this. The Desktop account host delegates to an installed
`bitfun` binary (including a package-manager symlink); if none is available,
probe reports the missing runner and submission remains disabled rather than
falling back to local execution.

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
deleted or archived.
Garbage collection never removes queued or running jobs. Removing a terminal
snapshot also removes only the managed directory bound to that job; an
arbitrary user-supplied target directory is never a cleanup target.

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
- A lost submit response leaves `submission_unknown`; status or an idempotent
  retry reconciles the target's durable truth.
- A live PID that no longer matches the exact worker command is never signaled
  and settles the job failed instead of leaving an orphaned non-terminal job.
- A target restart after the Runtime accepted a turn does not replay the prompt,
  because replay could duplicate tool side effects. The native target session
  remains available for manual resume.
- Prompt and event pages remain below the smallest host transport envelope.
- Workspace digest, archive traversal, unsupported entry, or size failures
  happen before job submission and leave no executable target workspace.
  Detached materialization failures are persisted and returned by later commit
  polls instead of being hidden in a discarded child-process stderr stream.
