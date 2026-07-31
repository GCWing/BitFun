# Dispatch Web UI

The Web UI supports local execution plus detached SSH and same-account device
dispatch.

## Invariants

1. A dispatch target is selected while creating a session and is immutable after
   the first turn.
2. `local` uses the existing session, worktree, persistence, and dialog-turn
   paths unchanged.
3. A non-local session is an observer projection. The controller must not call
   `create_session`, `bind_session_worktree`, `start_dialog_turn`, restore, or
   local session persistence for it.
4. The target CLI owns the ordinary durable session and the append-only event
   log. The controller owns only the outbound observer index and a UI cache.
   That UI cache is the observer transcript stored under
   `~/.bitfun/dispatch/outbound/.transcripts/<jobId>.json`. It holds the
   rendered projection, never a durable session, and the controller stores it
   verbatim without interpreting it.
5. Status cursors advance only after every returned event has been applied.
   Agent envelope ids are deduplicated before replay. Terminal jobs keep
   polling until an empty page confirms that the event log is fully drained. A
   persisted cursor is only reusable while its transcript cache is present and
   valid; the cursor recorded in that cache wins over any other stored cursor,
   because only those two were written together. Missing, corrupt, or
   version-mismatched cache means replay from byte zero.
6. SSH CLI installation is always a separate, explicit confirmation. The UI
   displays the resolved version, URL, and SHA256 before starting it.
7. Account devices use encrypted request/response RPC and distinct
   `dispatch_target_*` commands. They never attach Peer Device Mode and an
   offline target never falls back to local execution.
8. Approval policy is explicit per job: `auto`, `reject-and-report`, or
   `remote`. `remote` projects pending requests into the normal permission
   panel. The selected policy is visible in the normal session controls; submit
   must not add a second confirmation dialog.
9. MiniApp and quick-input hosts do not expose the dispatch picker.
10. Controller-side model settings never leak into an SSH dispatch. The submit
    omits `model` unless preflight recorded an explicit target model choice.
11. Deleting or archiving a projection writes a local job tombstone so outbound
    reconciliation cannot silently reopen it.
12. The observer ignores `SubagentSessionLinked`. Child observer ownership is not
    implemented, so creating an unmarked child projection would violate the
    observer-only persistence and cancellation boundary.
13. Workspace delivery is explicit. `existing` addresses a target directory;
    `snapshot-source` transfers tracked and non-ignored source without ignored
    build output or secrets; `snapshot-exact` transfers one verified source
    snapshot, including ignored and hidden regular files but excluding `.git`.
    Neither snapshot mode is live or bidirectional synchronization.
14. Cursor pulls are multi-observer safe. Truncation and omitted events are
    visible completeness facts and must not be rendered as a full transcript.
15. The observer continues bounded polling while the window is hidden so
    remote permission and terminal system notifications can be delivered.
16. Controller-wide outbound progress never advances a renderer's own cursor;
    each observer replays and commits only the events it applied.
17. Listing jobs for an explicitly selected target adopts only outbound
    observer routing records. It never restores the target session into the
    controller's backend store or acquires local runtime ownership.
18. Model configuration sync is a separate, explicit, credential-bearing
    operation with its own confirmation. It merges only the `ai` model keys
    into the target's `app.json`, preserves every other target setting, aborts
    rather than overwrite an unreadable or unparseable target config, and
    writes owner-only via a temp-file rename.
19. Dispatch target and status are session-scoped navigation metadata. Workspace
    navigation must not install a dispatch target or filter its session list by
    dispatch target.
20. The controller projects the initial user turn before waiting for target
    startup. The target's `DialogTurnStarted` event adopts that pending turn in
    place so queued work is visible without duplicating the message.
21. Every projected outbound observer record carries its durable
    controller-side source workspace identity. Legacy or adopted records
    without that identity remain hidden; the renderer must never guess
    ownership from whichever workspace initializes after restart. After submit
    acknowledgement, the controller index is authoritative and stale renderer
    cache without a matching record is pruned.
22. CLI compatibility is capability-based, not semver-only. A target must
    advertise safe CLI-profile selection for detached workers; development
    source updates use the clean controller commit only after the existing
    explicit source-build confirmation.
