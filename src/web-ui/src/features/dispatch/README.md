# Dispatch Web UI

The Web UI supports local execution plus detached SSH and same-account device
dispatch.

## Invariants

1. A dispatch target is selected while creating a session and is immutable after
   the first turn. The model and approval policy are not: protocol v4 carries
   them per follow-up turn, and the target persists the effective values onto
   the job. Because they are per-turn, neither is chosen in the setup dialog —
   both are ordinary composer controls, identical to a local session's. The
   setup dialog decides only what the target cannot change later: which target,
   which base revision, and whether uncommitted changes travel.
1b. A new dispatch session's approval policy is the one this device's own
   permission default implies (`ask` → `remote`, auto-approve or full access →
   `auto`). Because the policy is editable per turn, a target is only usable
   when it advertises all three approval capabilities, not just the current one.
1a. A dispatch session accepts follow-up messages. While a turn runs, a message
   is an `append` that steers it; once it has finished, a message is a
   `dispatch_continue` that queues the next turn against the same target
   session, worktree, and event log. The follow-up carries a caller-generated
   `turnId` so an ambiguous response cannot start two turns.
2. `local` uses the current session, worktree, persistence, and dialog-turn
   paths unchanged.
3. The dispatch picker follows the same Git-workspace visibility condition as
   the worktree control. A non-Git workspace cannot create a detached dispatch.
4. A non-local session is an observer projection. The controller must not call
   `create_session`, `bind_session_worktree`, `start_dialog_turn`, restore, or
   local session persistence for it.
5. The target CLI owns the ordinary durable session and the append-only event
   log. The controller owns only the outbound observer index and a UI cache.
   That UI cache is the observer transcript stored under
   `~/.openbitfun/dispatch/outbound/.transcripts/<jobId>.json`. It holds the
   rendered projection, never a durable session, and the controller stores it
   verbatim without interpreting it.
6. Status cursors advance only after every returned event has been processed.
   Agent envelope ids are deduplicated before replay. Terminal jobs keep
   polling until an empty page confirms that the event log is fully drained. A
   persisted cursor is only reusable while its transcript cache is present and
   valid; the cursor recorded in that cache wins over any other stored cursor,
   because only those two were written together. Missing, corrupt, or
   version-mismatched cache means replay from byte zero.
7. Every non-local job starts from a controller-owned managed baseline
   worktree. The outbound record stores its worktree id and path, base commit,
   branch, source workspace identity, remote URL when available, and last
   synchronized head.
8. The baseline is claimed by `dispatch:<jobId>` and is excluded from automatic
   worktree retention while the outbound job still depends on it. Cleanup must
   release the claim before deleting the outbound record. A failed release
   retains the record for retry so a claim cannot be orphaned; normal worktree
   retention resumes only after release.
9. Delivery is Git-only. The target maintains a shared bare repository cache at
   `~/.openbitfun/dispatch/repos/<repoKey>` and a job worktree at
   `~/.openbitfun/dispatch/worktrees/<jobId>`. A missing base commit is supplied by
   a SHA-256-bound, Git-verified bundle; no origin write permission is required.
10. The setup dialog accepts a base revision (`HEAD` by default); the
    controller resolves it once when creating the managed baseline.
    `includeUncommitted` copies and commits Git-visible controller changes only
    inside the managed baseline. The user's checkout is never changed, and
    ignored runtime inputs are not sent to the target.
11. SSH submission automatically installs or upgrades a compatible signed
    prebuilt `openbitfun` release when the target runner is missing or incompatible.
    The resolved version, URL, and SHA-256 remain visible, integrity checks stay
    mandatory, installation has a bounded deadline, and submission probes the
    installed runner again before provisioning. Each `cli-install` audit
    transition is durably written to the preparation journal; the started event
    is persisted before remote installer mutation, and an audit write failure
    stops submission. The events are projected into the pending Dispatch turn
    and cached with the transcript, so the automatic action remains visible
    after replay or controller restart.
12. The signed prebuilt release is the only way a target gets a runner. The
    controller never compiles OpenBitFun on someone else's machine, so a target no
    published binary fits is reported as unusable rather than offered a build.
13. Account devices use encrypted request/response RPC and distinct
   `dispatch_target_*` commands. They never attach Peer Device Mode and an
   offline or incompatible target never falls back to local execution. Device
   dispatch does not install software through the Relay.
14. Approval policy is explicit and editable between turns: `auto`,
   `reject-and-report`, or `remote`. `remote` projects pending requests into
   the normal permission panel. The selected policy is visible in the normal
   session controls; submit must not add a second confirmation dialog.
15. MiniApp and quick-input hosts do not expose the dispatch picker.
16. The model picker offers this controller's own catalog, because submission
    guarantees the target can serve whatever it offers (invariant 25). The
    target's probed list and default are a starting point, unioned in rather
    than authoritative, so a projection restored without that snapshot still
    has a working picker. Submit omits `model` only while the session has no
    explicit choice, leaving the target on its own default.
17. One-click synchronization is available from `running` through terminal
    states. It commits target changes when needed, validates that both managed
    worktrees remain on the named job branch, and verifies the returned Git
    bundle. The first bundle covers `baseCommit..<branch>`; later bundles cover
    `<knownHead>..<branch>`, where `knownHead` is the last successfully stored
    head and must be an ancestor of the current branch head. Only the controller
    baseline advances. The user's checkout is never changed; users merge or
    rebase the dispatch branch with ordinary Git tools after review.
18. Synchronization has no path-overwrite or conflict-resolution mode. A
    missing baseline worktree is shown directly in the UI, and a divergent
    baseline or branch mismatch fails closed rather than being reset. A running
    checkpoint that meets a transient Git or index lock is retryable, leaves
    `knownHead` unchanged, and can be repeated after the lock clears.
19. Deleting or archiving a projection writes a local job tombstone so outbound
    reconciliation cannot silently reopen it.
20. Subagent sessions linked under a dispatch job flow through the event log
    and render as child projections. Ownership is driver-resolved through the
    parent chain: a child of a projection is itself observer-only (never
    persisted locally, never driven as a local backend session).
21. Cursor reads are multi-observer safe. Truncation and omitted events are
    visible completeness facts and must not be rendered as a full transcript.
22. The observer continues bounded polling while the window is hidden so
    remote permission and terminal system notifications can be delivered.
23. Controller-wide outbound progress never advances a renderer's own cursor;
    each observer replays and commits only the events it processed.
24. Listing jobs for an explicitly selected target adopts only outbound
    observer routing records. It never restores the target session into the
    controller's backend store or acquires local runtime ownership.
25. SSH submission pushes this controller's model configuration to a target
    that cannot serve the submission's model, before creating any baseline, and
    re-probes. Choosing the target is the consent: it is the same credential
    write the manual command performs, and it stays visible as `model-sync`
    rows in the preparation journal and the projected transcript, exactly like
    `cli-install`. It merges only the `ai` model keys into the target's
    `app.json`, preserves every other target setting, aborts rather than
    overwrite an unreadable or unparseable target config, and writes
    owner-only via a temp-file rename. A failed push is not fatal by itself:
    the submission then reports the target's own model diagnostic. Device
    targets have no such repair path and still fail closed.
25a. Setup-audit rows are forwarded only when the target advertises the
    matching capability. A target rejects an unknown audit action outright, so
    `model-sync` rows are dropped for a CLI predating
    `setup_audit_model_sync`; the controller journal keeps them regardless.
26. Dispatch target and status are session-scoped navigation metadata. Workspace
    navigation must not install a dispatch target or filter its session list by
    dispatch target.
27. The controller projects the initial user turn before waiting for target
    startup. The target's `DialogTurnStarted` event adopts that pending turn in
    place so queued work is visible without duplicating the message.
28. Every projected outbound observer record carries its durable
    controller-side source workspace identity. Legacy or adopted records
    without that identity remain hidden; the renderer must never guess
    ownership from whichever workspace initializes after restart. After submit
    acknowledgement, the controller index is authoritative and stale renderer
    cache without a matching record is pruned.
29. CLI compatibility is protocol-v3 and capability-based, not semver-only. A
    target must advertise Git worktree delivery, bundle upload and
    synchronization, workspace serialization, and safe CLI-profile selection
    for detached workers.
30. Before submit acknowledgement, controller setup is recoverable from
    `~/.openbitfun/dispatch/outbound/.preparations/<jobId>.json`. The journal is
    retained through the validated target acknowledgement, and preparation,
    retry, and recovery for one job share a per-job run lock.
31. A preparation records the stable project path that owns the worktree
    registry before creating `dispatch:<jobId>`. Recovery releases that claim
    only after the preparation lease expires and no matching outbound record
    owns the baseline. A matching owner or an owner-read error conservatively
    retains the claim and journal for retry.
32. Dispatch target options use SSH connection identity and display metadata
    only. A saved SSH `defaultWorkspace` is not offered as the job's execution
    directory; the target always provisions the job's managed Git worktree.
