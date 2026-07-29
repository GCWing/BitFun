# Dispatch Web UI

Phase one supports local execution and detached SSH dispatch only.

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
5. Status cursors advance only after every returned event has been applied.
   Agent envelope ids are deduplicated before replay. Terminal jobs keep
   polling until an empty page confirms that the event log is fully drained.
6. SSH CLI installation is always a separate, explicit confirmation. The UI
   displays the resolved version, URL, and SHA256 before starting it.
7. Phase one never lists or routes to account devices. Peer Device Mode keeps
   every dispatch command on the controller because SSH credentials live there.
8. Unattended approval policy is explicit per job: `auto` or
   `reject-and-report`. There is no implicit interactive mode. `auto` also
   requires a one-shot, non-persisted confirmation immediately before submit.
9. MiniApp and quick-input hosts do not expose the dispatch picker.
10. Controller-side model settings never leak into an SSH dispatch. The submit
    omits `model` unless preflight recorded an explicit target model choice.
11. Deleting or archiving a projection writes a local job tombstone so outbound
    reconciliation cannot silently reopen it.
12. Phase one ignores `SubagentSessionLinked`. Child observer ownership is not
    implemented, so creating an unmarked child projection would violate the
    observer-only persistence and cancellation boundary.
