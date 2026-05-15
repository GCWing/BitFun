# MiniApp Customization Sandbox Design

## Summary

BitFun should let users customize any MiniApp from the running app surface, while keeping the actual app reversible and protected from accidental AI edits.

The product behavior is:

- The user opens any MiniApp and triggers a host-level customization entry.
- BitFun creates an isolated draft from the current app.
- The user chats with an assistant about changes such as lighter colors, extra local stats, or a new export button.
- The assistant edits only the draft.
- BitFun previews the draft in a sandbox runner.
- The user explicitly applies or discards the draft.
- Applying saves the previous active app as a rollback version before replacing the active source.

This applies uniformly to built-in apps, imported local apps, and user-created apps. Five-in-a-row is only an example.

## Goals

- Provide one customization workflow for every MiniApp.
- Preserve the user's mental model that they are modifying the current app, not creating a separate duplicate.
- Ensure AI-generated changes cannot affect the active app until the user confirms.
- Keep draft storage isolated from the active app's `storage.json`.
- Preserve rollback for every applied customization.
- Prevent BitFun version upgrades from silently overwriting customized built-in MiniApp source.
- Show clear risk and permission prompts before changes become active.
- Fit the implementation into no more than two pull requests.

## Non-goals

- No MiniApp marketplace, sharing, syncing, or cloud backup.
- No automatic merge of customized built-in apps with newer official built-in versions.
- No direct `app.bitfun.*`, workspace service, session service, Git service, or terminal service API inside MiniApps.
- No per-MiniApp custom implementation of the customization entry.
- No irreversible direct write path from the assistant to the active app.

## Current Foundations

MiniApp V2 already has most of the primitives needed for this feature:

- App files are stored under `PathManager::miniapps_dir()`, currently `user_data_dir()/miniapps`.
- Each app has `meta.json`, `source/`, `package.json`, `compiled.html`, `storage.json`, and optional `versions/`.
- `MiniAppManager::update` saves the previous app version before writing a new active version.
- `MiniAppManager::rollback` restores a saved version as a new active version.
- `MiniAppRunner` runs compiled HTML in a sandboxed iframe.
- `useMiniAppBridge` routes `window.app` calls to storage, host dispatch, worker calls, dialog calls, theme changes, and locale changes.
- Built-in apps are seeded into the same local MiniApp directory and preserve `storage.json` during reseed, but currently overwrite source files when the bundled built-in version increases.
- `InitMiniApp` already creates local MiniApp skeletons that agents can edit using normal file tools.

The new design should build on these instead of creating a separate app format.

## Concepts

### Active App

The currently installed MiniApp shown in the gallery and opened by `miniapp:{app_id}` scene tabs. Active app files remain in the existing directory:

```text
miniapps/{app_id}/
  meta.json
  source/
  package.json
  compiled.html
  storage.json
  versions/
```

### Draft

A temporary, isolated copy of an app being customized. Drafts are hidden from normal gallery listing by placing them under a dot directory:

```text
miniapps/.drafts/{app_id}/{draft_id}/
  draft.json
  meta.json
  source/
  package.json
  compiled.html
  storage.json
```

The draft has its own storage file. Preview writes never touch `miniapps/{app_id}/storage.json`.

### Local Override

A marker that says the active app source is now user-owned and must not be silently overwritten by built-in reseed logic:

```text
miniapps/{app_id}/.customization.json
```

Example:

```json
{
  "origin": {
    "kind": "builtin",
    "builtin_id": "builtin-gomoku",
    "builtin_version": 11
  },
  "local_override": true,
  "last_applied_draft_id": "draft-20260515-abc123",
  "updated_at": 1778784000000
}
```

Imported and user-created apps can also have customization metadata, but `local_override` only affects built-in reseed behavior.

### Built-in Baseline

When a customized built-in app receives a newer bundled version, BitFun should not overwrite the active source. Instead, it records that a newer official baseline exists. The first version can store this as metadata only:

```json
{
  "available_builtin_update": {
    "builtin_version": 12,
    "detected_at": 1778784000000
  }
}
```

The UI can later offer "view official update", "keep my version", or "replace with official version". Automatic merge is out of scope.

## Draft Lifecycle

### 1. Trigger

The customization entry belongs to the MiniApp host surface, not to the MiniApp iframe.

Supported first-version triggers:

- A small scene header icon.
- A keyboard shortcut while the MiniApp scene is focused.
- A hover hotspot near a stable MiniApp viewport corner.

The host entry opens a customization panel and shows a risk notice before creating or editing a draft.

### 2. Create Draft

BitFun copies the active app source and metadata into `miniapps/.drafts/{app_id}/{draft_id}/`.

Draft creation must:

- Copy `meta.json`, `source/`, `package.json`, and dependency metadata.
- Create a draft-local `storage.json`.
- Compile draft HTML using the draft source and active app permissions.
- Store the original active app version and source revision in `draft.json`.
- Emit a `miniapp-draft-created` event.

The active app is not changed.

### 3. Assistant Edits Draft

The assistant receives a prompt that includes:

- App name and id.
- Draft id.
- Draft root path and editable files.
- A hard instruction to edit only files under the draft root.
- A warning that active app files must not be changed.
- The current MiniApp capability limits.
- The user's request.

First version should reuse the existing side-thread / auxiliary-pane chat flow instead of building a second full chat system inside the MiniApp scene. The MiniApp scene owns the customization launcher and draft preview; the chat itself can run in the existing right-panel session UI.

### 4. Preview Draft

The draft preview iframe uses draft-compiled HTML and draft-local storage.

Preview must:

- Use the same sandbox policy as active MiniApp rendering.
- Use draft storage for `app.storage.get/set`.
- Recompile from draft source when the user requests refresh or when the assistant reports changes.
- Stop draft workers separately from active app workers.
- Never call `update_miniapp` on the active app.

### 5. Permission Diff

Before applying, BitFun compares active permissions with draft permissions.

Low-risk changes include UI source, metadata, tags, and app text.

High-risk changes include:

- Adding or expanding `fs.write`.
- Adding or expanding `fs.read` outside `{appdata}`.
- Adding `shell.allow` entries.
- Expanding `net.allow`.
- Enabling `node.enabled`.
- Adding `npm_dependencies`.
- Enabling `ai.enabled` or increasing AI limits.

Any high-risk permission change requires a distinct confirmation step before `Apply` is allowed.

### 6. Apply Draft

Applying a draft is the only path that writes draft source into the active app.

Apply order:

1. Stop the active worker if needed.
2. Load the active app.
3. Save the active app as a version snapshot.
4. Copy draft source, metadata fields, package data, permissions, and AI context into the active app.
5. Increment active app version.
6. Mark built-in apps as local override.
7. Recompile active app.
8. Save active app.
9. Emit `miniapp-draft-applied` and `miniapp-updated`.
10. Keep or delete the draft according to the request.

Active `storage.json` remains active storage. Draft storage is not migrated by default.

### 7. Discard Draft

Discard removes the draft directory and stops any draft worker. It never touches active app source or storage.

### 8. Rollback

Rollback keeps using the existing MiniApp version mechanism. A rollback after an applied customization restores the previous app source as a new current version.

If the active app is a customized built-in app, rollback does not remove the local override marker unless the user explicitly chooses "restore official version".

## Built-in Upgrade Behavior

Current built-in seeding overwrites source and metadata when the bundled built-in version is newer, while preserving `storage.json`.

After this feature:

- If no `.customization.json` local override exists, reseed behavior may stay the same.
- If local override exists, reseed must not overwrite active `source/`, `meta.json`, `package.json`, or `compiled.html`.
- Reseed should record the available official update in customization metadata.
- The user's active app stays unchanged after BitFun upgrades.

This makes "directly modifying the current app" safe for built-in apps without forcing a visible duplicate in the gallery.

## Risk And Peripheral Impact Analysis

### Data Layout And Migration Risk

MiniApp files already live under the user data directory. Adding `.drafts` and `.customization.json` must not change the existing visible app directory contract. Existing apps should keep loading even if the new metadata files are absent, corrupt, or partially written.

Mitigation:

- Treat missing customization metadata as "not customized".
- Keep drafts under a dot directory so `list_miniapps` does not show them.
- Make draft discard idempotent.
- Never require a migration step before existing apps can launch.

### Built-in Upgrade Risk

The main behavior change is that customized built-in apps stop receiving automatic source overwrites. This protects user changes, but it can also leave users on older official built-in code.

Mitigation:

- Record the available official built-in version when reseed is skipped.
- Keep active source unchanged by default.
- Defer merge/replace UI to a later explicit product decision.
- Keep storage preservation behavior unchanged.

### Active/Draft Cross-contamination Risk

The largest correctness risk is accidentally routing draft preview calls into active app storage, active app source, or active workers. That would make the sandbox misleading.

Mitigation:

- Introduce an explicit run scope for active vs draft.
- Keep draft `storage.json` in the draft directory.
- Use a draft-specific worker key and draft app directory.
- Keep active `app_id` as the logical id passed to the MiniApp bridge, while backend routing uses the run scope.

### Permission Escalation Risk

Assistant-generated changes may add file, shell, network, Node, npm, or AI access. If BitFun applies those without review, the customization flow becomes a permission escalation path.

Mitigation:

- Compute a structured permission diff before Apply.
- Classify added or expanded `fs.read`, `fs.write`, `shell.allow`, `net.allow`, `node.enabled`, and `ai.enabled` as high risk.
- Require a separate confirmation for high-risk changes in PR 2.
- Keep PR 1 permission diff in pure product-domain helpers so UI wording cannot become the policy source.

### Worker Runtime Risk

Node/Bun worker runtime currently keys workers by app id and derives the app directory from `miniapps/{app_id}`. Draft preview cannot reuse that path.

Mitigation:

- Add worker-pool support for an explicit worker key and app directory.
- Use a draft worker key such as `{app_id}:draft:{draft_id}`.
- Stop draft workers independently from active workers.
- Keep existing active worker commands unchanged.

### Remote Workspace Impact

MiniApp customization is local app-data behavior, while workspace access may point at local or remote workspaces. Draft source and storage should remain local to the BitFun client. Workspace access inside MiniApps still follows the existing `{workspace}` permission model.

Mitigation:

- Keep draft directories under local user data.
- Pass `workspace_path` only to compile/policy resolution just like current MiniApp APIs.
- Do not introduce remote synchronization for drafts.
- If a remote workspace path is unavailable or unsupported, fail the affected app call clearly instead of applying draft changes.

### AI Session And Tooling Impact

The customization assistant must edit draft files only. If it receives active app paths, it may bypass the sandbox.

Mitigation:

- The prompt must include only draft editable paths.
- The prompt must explicitly forbid active app edits.
- Apply is the only active write path.
- PR 2 should reuse existing side-thread UI rather than adding a new agent runtime.

### Gallery And Existing MiniApp UX Impact

Drafts must not appear as separate gallery apps. Users should feel they are editing the current app, not managing duplicate app cards.

Mitigation:

- Hide `.drafts` from `list_app_ids`.
- Keep active app id and scene id stable after Apply.
- Emit normal `miniapp-updated` events after Apply so existing gallery refresh behavior continues.

### Rollback Semantics Impact

Applying a draft creates a new active version. Rollback should restore source and metadata but should not delete customization metadata automatically, because the user may still be on a customized built-in lineage.

Mitigation:

- Save active version before Apply.
- Keep active `storage.json` unchanged during Apply.
- Preserve local override metadata unless the user chooses an official restore flow.

### Verification Impact

The feature spans product-domain helpers, core storage, desktop commands, and web API typing. A partial pass can easily miss cross-layer drift.

Minimum PR 1 verification:

- `cargo test -p bitfun-product-domains --features miniapp customization`
- `cargo test -p bitfun-core miniapp -- --nocapture`
- `cargo check --workspace`
- `pnpm run type-check:web`

## Host UI Model

The MiniApp iframe should not know this feature exists. The host wraps the iframe with customization affordances:

- Header icon for discoverability.
- Shortcut for power users.
- Hover hotspot for low-friction access.
- Customization panel that contains risk notice, chat launch, draft status, preview controls, permission diff, apply, discard, and rollback entry points.

The first version can use the existing auxiliary chat panel for the conversation. The MiniApp scene remains the control center for draft status and preview.

## Risk Notice Copy Requirements

The notice should be calm and specific:

- The assistant will modify a draft copy of this MiniApp.
- Changes are previewed in a sandbox first.
- Nothing changes in the real app until the user applies the draft.
- BitFun saves the current version before applying.
- New permissions such as file writes, shell commands, network access, Node runtime, npm packages, or AI access require extra confirmation.

## Backend API Shape

The final names can follow existing Tauri command conventions, but the first implementation should expose this capability as explicit MiniApp draft APIs:

| Command | Purpose |
|---|---|
| `create_miniapp_draft` | Copy active app into a draft and compile it |
| `get_miniapp_draft` | Return draft metadata and compiled preview HTML |
| `miniapp_draft_sync_from_fs` | Reload draft source from disk and recompile |
| `miniapp_draft_set_permissions` | Update draft permissions when an assistant-generated change requires it |
| `get_miniapp_draft_permission_diff` | Compare draft and active permissions |
| `get_miniapp_draft_storage` | Read a draft-local storage key |
| `set_miniapp_draft_storage` | Write a draft-local storage key |
| `miniapp_draft_host_call` | Route host primitives through draft policy and draft storage |
| `miniapp_draft_worker_call` | Route worker calls through a draft worker scope |
| `miniapp_draft_worker_stop` | Stop a draft worker without stopping the active app worker |
| `apply_miniapp_draft` | Snapshot active app, apply draft, recompile active app |
| `discard_miniapp_draft` | Stop draft runtime and remove draft files |

Draft storage and runtime calls need a scope:

```ts
type MiniAppRunScope =
  | { kind: 'active'; appId: string }
  | { kind: 'draft'; appId: string; draftId: string };
```

The bridge can keep presenting the logical active `appId` to MiniApp code, while the host uses `MiniAppRunScope` to route storage, worker, and host calls to the active or draft directory.

## Frontend API Shape

`MiniAppAPI.ts` should mirror the backend APIs with typed requests and responses:

```ts
export interface MiniAppDraft {
  appId: string;
  draftId: string;
  sourceVersion: number;
  compiledHtml: string;
  permissions: MiniAppPermissions;
  status: 'draft' | 'applied' | 'discarded';
}

export interface MiniAppPermissionDiff {
  highRisk: boolean;
  added: string[];
  expanded: string[];
  removed: string[];
}
```

The UI should not infer permission risk from display strings. It should use structured diff data from the backend or shared pure helpers.

## Testing Strategy

Backend tests:

- Draft creation copies source and uses isolated storage.
- Draft preview does not change active `storage.json`.
- Applying a draft creates a version snapshot before active source changes.
- Discarding a draft leaves active app unchanged.
- Built-in reseed skips source overwrite for local override apps.
- Permission diff flags high-risk expansions.

Frontend tests:

- The host customization trigger appears for every MiniApp.
- Shortcut and hotspot open the same customization state.
- Risk notice appears before chat launch.
- Apply is disabled until preview exists.
- High-risk permission diff shows a second confirmation.
- Discard closes the draft state without closing the active MiniApp scene.

Verification:

- `cargo test -p bitfun-core miniapp`
- `cargo test -p bitfun-product-domains --features miniapp`
- `cargo check --workspace`
- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run`

## PR Budget

This feature must ship in no more than two PRs:

1. **PR 1: MiniApp draft sandbox and local override foundation**
   - Backend storage, manager, API, permission diff, and built-in reseed protection.
   - Minimal TypeScript API typing if needed for compile boundaries.
   - No full customization UI.

2. **PR 2: Host customization entry and assistant workflow**
   - MiniApp scene trigger, risk notice, side-chat launch, draft preview, permission confirmation, apply, discard, and locale coverage.

If scope grows beyond these two PRs, defer lower-value polish rather than creating a third PR.
