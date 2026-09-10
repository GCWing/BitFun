# Skill groups

This feature owns reusable skill group definitions, their resolution against a
skill catalog, and the shared configuration store. Groups keep the existing
version-one `app.user_skill_groups` shape (`id`, `name`, `skillKeys`).

- The Skills scene creates, edits, copies, reorders, and deletes groups. Built-in
  groups come from catalog metadata and are read-only; copying creates a user group.
- The Agent detail picker reads groups and expands selections into the Agent's
  final skill keys. Editing or deleting a group never changes Agent configuration,
  global skill availability, installed files, or runtime policy.
- Empty groups and unavailable member references remain visible and editable.
  An incomplete catalog must not prune references or imply successful discovery.
- The store follows the existing device surface activation scope. It shares loads,
  rejects stale writes after a device switch, and serializes mutations in this
  window. Each mutation re-reads persisted data and checks the edited group's
  original value; unrelated group edits are preserved. This is not a cross-process
  compare-and-swap contract.
- Missing legacy configuration defaults to an empty collection. Invalid data or
  an unsupported version is an error and must not be rewritten as empty data.

Catalog availability is scoped to the device and workspace. The Skills scene
keeps its existing explicit unsupported state for Peer Device Mode and non-desktop
hosts. Group definitions are UI preferences, so remote control and detached jobs
continue consuming the existing flattened Agent configuration without a group
resolution dependency on the controller.

Focused verification:

```bash
pnpm --dir src/web-ui run test:run src/features/skill-groups/skillGroups.test.ts src/features/skill-groups/skillGroupsStore.test.ts
```

These are data and lifecycle contracts. They do not establish visual fidelity or
live SSH, Peer, remote-control, or detached-dispatch behavior.
