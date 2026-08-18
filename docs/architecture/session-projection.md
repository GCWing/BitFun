# Session Projection

What a client shows for a Session is a **projection of an ordered stream**. This
document is the contract that projection obeys, and the migration that brings
the existing writers under it.

Read [`peer-device-mode.md`](peer-device-mode.md) for how a controller reaches
another device. This document is about what happens to the data once it
arrives, and applies identically on the local surface.

## The problem this replaces

Seven independent writers currently produce a Session's on-screen state:

| # | Writer | Entry point |
|---|---|---|
| 1 | Live agentic events | `AgenticEventListener` → `eventBatcher` |
| 2 | Disk hydrate | `loadSessionHistory` → `restore_session_view` |
| 3 | Windowed history | `load_session_turn_window` |
| 4 | Snapshot reconcile | `refreshPeerSessionSnapshot`, `replaceRunningSnapshot` |
| 5 | Journal snapshot replay | `dispatchExternal(snapshot.events)` |
| 6 | Interaction mailbox | `reconcilePendingUserQuestions` |
| 7 | Backfill delta | `load_session_event_backfill` |

None of them carries a position that the others can compare against, so every
pair needs its own conflict rule. Those rules are the fourteen invariants in
[`peer-device/README.md`](../../src/web-ui/src/infrastructure/peer-device/README.md),
and they are written in terms of painted content rather than ordering:
`snapshotDropsProjectedTurnContent` decides whether a write is safe by counting
rounds and progress entries; `runtimeProjectionCaughtUp` decides whether a
cursor may be trusted by looking for tool cards on screen.

Counting pixels to decide whether a write is safe is what a missing position
looks like. The rule table also grows quadratically: adding writer 7 required
two new pairwise rules (7↔1, 7↔6), and shipping without them produced exactly
two defects — the live stream stalled behind writer 7's fence, and a blocking
interaction came back unanswerable because writer 6 never ran.

## Contract

### 1. Every write carries a position, and the projection never regresses

A write whose position is not ahead of what has been applied is **dropped, not
merged**. There is no operation that replaces projected content, so no writer
needs to prove it is not about to lose any.

Positions are per `(surface, session)`:

- **Runtime positions** — `(streamId, cursor)`, minted by the Host journal as
  each event enters its ordered delivery stream. `streamId` identifies the
  Runtime process; cursors from different `streamId`s are never comparable.
- **History positions** — turn ordinal within the persisted record. Immutable
  and totally ordered.

The two are not compared with each other. They cannot conflict, because of
invariant 2.

### 2. A Turn has exactly one writer, decided by whether it is executing

- An **executing** Turn is owned by the runtime stream. No persisted record,
  checkpoint, or snapshot of that Turn may write it.
- A **settled** Turn is owned by the persisted record. No live event may
  write it.
- A terminal event starts client settlement, but it is not proof that the
  terminal record is durable. Ownership transfers **once**, at the Runtime's
  post-persistence history fence, driven by position — never by inspecting
  what is on screen.

This is why history and live events cannot race: they are never both
authoritative for the same Turn. The persisted checkpoint of an executing Turn
is identity only; it names the Turn and carries none of its content.

Native Runtime completion is rebuilt from the complete generation journal under
the same per-Session mutation lock used by projected saves. A projected
checkpoint may contribute additive display metadata, but it cannot shorten
canonical text or thinking, remove a round or tool result, or turn a settled
record back into an executing one. Externally projected sessions such as ACP
remain owned by their external projection.

After the terminal record is committed, `SessionHistoryChanged` is the durable
fence. Local and Peer Device surfaces re-read the affected tail Turn. Remote
Connect polling sends an optional authoritative `message_snapshot`, because an
already-counted assistant message can grow from a streamed prefix to its full
persisted content without changing message count. New clients accept the
snapshot; older clients continue to use the existing additive fields.

### 3. Identity is `(surface, session)` by construction

One `SessionStream` object owns the position, the pending queue, and the
projection for one `(DeviceSurfaceId, SessionId)`. Workspace paths and session
ids repeat across machines, so surface is part of identity, not an extra
argument each feature remembers to thread through.

Any state that is per-Session is reached through its stream. A feature cannot
hold Session state that is not surface-scoped, because there is nowhere to
put it.

## Sources are not writers

Every source above becomes a way of **obtaining positioned events**, applied
through one path:

| Source | Produces |
|---|---|
| Live DeviceEvent / local emit | events at `(streamId, cursor)` |
| `load_session_event_backfill` | events after a position, or `snapshotRequired` |
| `restore_session_view` runtime snapshot | a compacted prefix ending at a position |
| `restore_session_view` turns / `load_session_turn_window` | settled Turns at history positions |
| Interaction mailbox | revisioned state of an executing Turn, applied at its position |

A snapshot is a prefix. A delta is a suffix. History is the older part of the
same order. None of them is a distinct kind of write.

## What this deletes

Each item disappears when its writer migrates. This list is the acceptance
criteria — a migration step that does not remove its entry has not finished.

| Removed | Replaced by |
|---|---|
| `replaceRunningSnapshot` | there is no replace operation (contract 1) |
| `runtimeProjectionCaughtUp` | the applied position is the answer (contract 1) |
| `prepareRuntimeTurnReplay` / `asRuntimeReplayTurn` | an executing Turn has one writer (contract 2) |
| `hasGap` / `projectionStale` / `markRuntimeSessionProjectionStale` | a position discontinuity is the gap |
| `beginRuntimeSessionAttachment` fence | the stream's own queue (contract 3) |
| manual `(DeviceSurfaceId, …)` threading | stream identity (contract 3) |

The 3s poll is **not** on this list. Events that never arrive advance no
cursor, so no discontinuity is observable; the poll remains the liveness probe
that notices a stream has gone quiet. It stops being a repair mechanism.

### Two gaps the contract does not yet close

Both were found by deleting a heuristic and watching a behavioural test fail.
They are why `snapshotDropsProjectedTurnContent` and
`isRunningSnapshotForwardProgress` survive, inside
`persistedReadMayReplaceTurn`, as the last content comparison in the merge:

- **A Host that serves no runtime projection.** Contract 2 hands an executing
  Turn to the runtime stream, but an older Host has no such stream. Its
  persisted checkpoint is the only progress that exists, so forward progress
  from it is still admitted when `runtimeEventSnapshot` is absent.
- **A partial history read.** History positions are turn ordinals, and a
  windowed or not-yet-checkpointed read can name a Turn while carrying none of
  its work. Such a read holds no position for the content it omitted, so
  writing the Turn from it is lossy rather than advancing. Closing this needs
  the read to report its own completeness; until then "would this write lose
  content" is the only question available. The comparison includes content
  prefix progress and completed tool results, not only round or item counts.

Deleting either guard without first closing its gap reintroduces a real defect,
not just a test failure.

## Migration

Ordered so that each step is separately verifiable and deletes its own rules.

1. **Position algebra + `SessionStream`** — the contract as a tested module,
   with no writer on it yet.
2. **Runtime-stream writers (1, 5, 7)** — live events, snapshot replay, and
   backfill are already positioned; move them onto the stream and delete the
   fence, `hasGap`, and `runtimeProjectionCaughtUp`.
3. **Snapshot reconcile (4)** — becomes "apply a prefix"; deletes
   `replaceRunningSnapshot` and `snapshotDropsProjectedTurnContent`.
4. **Interaction mailbox (6)** — applied at the executing Turn's position
   rather than as a separate reconcile pass.
5. **History (2, 3)** — settled Turns at history positions; deletes the
   executing/settled overlap rules and `prepareRuntimeTurnReplay`.

Steps 2–5 each remove entries from the peer-device README's invariant list.
That list shrinking is the measure of progress; if it is not shrinking, the
step reintroduced a pairwise rule instead of removing one.

## Host contract

Hosts expose exactly two reads over a Session's stream, both already present:

- `restore_session_view` — a prefix (compacted projection + settled Turns +
  mailbox), ending at a position.
- `load_session_event_backfill` — the suffix after a position, or
  `snapshotRequired` when contiguity cannot be proven.

`SessionEventJournal` owns both. The compacted projection answers "what does
this Turn look like now"; the append-only tail answers "what came after
position N". Neither is allowed to answer the other's question — that
conflation is what made a gap something to infer.

Peer ownership is a cancellation and bookkeeping boundary and never filters
either read. A Turn started in a Host's own TUI is part of the Session every
attached surface projects.
