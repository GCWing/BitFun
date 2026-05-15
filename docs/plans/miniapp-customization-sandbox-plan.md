# MiniApp Customization Sandbox Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users customize any MiniApp through a host-level assistant flow that edits a draft sandbox first, then applies confirmed changes to the current app with rollback and built-in upgrade protection.

**Architecture:** Keep active MiniApps in the existing local app directory, add hidden draft directories under `miniapps/.drafts/{app_id}/{draft_id}`, and route draft preview through an explicit run scope so storage and runtime calls stay isolated. Ship in two PRs: backend foundation first, then host UI and assistant workflow.

**Tech Stack:** Rust `bitfun-core` MiniApp manager/storage, `bitfun-product-domains` pure permission diff helpers, Tauri commands in `src/apps/desktop`, React MiniApp scene UI, existing Flow Chat side-thread panel, Vitest, Rust unit tests.

---

## PR Limit

This work must not exceed two PRs.

- **PR 1:** backend foundation: draft sandbox, permission diff, local override, Tauri API.
- **PR 2:** frontend product surface: trigger, risk notice, assistant launch, preview, apply/discard.

If a task does not fit, defer it to a later separately approved roadmap item instead of opening a third PR.

## Files And Responsibilities

### Backend Foundation

- Create `src/crates/product-domains/src/miniapp/customization.rs`
  - Pure data structures and helpers for customization metadata and permission risk diff.
- Modify `src/crates/product-domains/src/miniapp/mod.rs`
  - Export the new `customization` module behind the existing MiniApp feature.
- Modify `src/crates/product-domains/src/miniapp/types.rs`
  - Only if shared permission helper types need to reference existing permission structs.
- Modify `src/crates/core/src/miniapp/storage.rs`
  - Add hidden draft path helpers, draft load/save, draft storage load/save, and local override metadata IO.
- Modify `src/crates/core/src/miniapp/manager.rs`
  - Add draft lifecycle methods: create, get, sync, permission diff, apply, discard.
- Modify `src/crates/core/src/miniapp/builtin/mod.rs`
  - Skip active source overwrite for local override built-in apps and record available official update.
- Modify `src/crates/core/src/miniapp/mod.rs`
  - Re-export new public draft/customization types if needed by desktop API.
- Modify `src/apps/desktop/src/api/miniapp_api.rs`
  - Add Tauri commands for draft lifecycle, permission diff, draft storage, and draft runtime calls.
- Modify `src/apps/desktop/src/lib.rs` or command registration file
  - Register the new Tauri commands.
- Modify `src/web-ui/src/infrastructure/api/service-api/MiniAppAPI.ts`
  - Add TypeScript request/response types and API wrappers for PR 2 to consume.

### Frontend Product Surface

- Create `src/web-ui/src/app/scenes/miniapps/customization/miniAppCustomizationTypes.ts`
  - UI-only state types for draft status, panel state, and risk notice state.
- Create `src/web-ui/src/app/scenes/miniapps/customization/miniAppCustomizationPrompt.ts`
  - Builds the assistant prompt with draft paths and strict edit boundaries.
- Create `src/web-ui/src/app/scenes/miniapps/customization/MiniAppCustomizeEntry.tsx`
  - Header button, shortcut, and hotspot trigger.
- Create `src/web-ui/src/app/scenes/miniapps/customization/MiniAppCustomizePanel.tsx`
  - Risk notice, draft controls, chat launch, preview refresh, apply, discard.
- Create `src/web-ui/src/app/scenes/miniapps/customization/MiniAppDraftPreview.tsx`
  - Sandboxed draft iframe using the same runner semantics with draft run scope.
- Create `src/web-ui/src/app/scenes/miniapps/customization/MiniAppPermissionDiffDialog.tsx`
  - Structured permission diff and high-risk confirmation.
- Create `src/web-ui/src/app/scenes/miniapps/customization/useMiniAppCustomizeHotspot.ts`
  - Focused shortcut and hover hotspot behavior.
- Modify `src/web-ui/src/app/scenes/miniapps/components/MiniAppRunner.tsx`
  - Accept an optional run scope for active vs draft preview.
- Modify `src/web-ui/src/app/scenes/miniapps/hooks/useMiniAppBridge.ts`
  - Route storage, worker, and host calls through active or draft run scope.
- Modify `src/web-ui/src/app/scenes/miniapps/MiniAppScene.tsx`
  - Host the entry, panel, and draft preview state.
- Modify `src/web-ui/src/app/scenes/miniapps/MiniAppScene.scss`
  - Style the entry, hotspot, panel, and preview layout.
- Modify locale files under `src/web-ui/src/locales/{en-US,zh-CN,zh-TW}/scenes/miniapp.json`
  - Add risk, permissions, apply, discard, and trigger copy.

## PR 1: MiniApp Draft Sandbox And Local Override Foundation

Risk: Medium. This touches MiniApp storage and built-in reseed behavior, but has a narrow product surface and can be tested without UI.

### Task 1: Add Pure Customization Types And Permission Diff

**Files:**
- Create: `src/crates/product-domains/src/miniapp/customization.rs`
- Modify: `src/crates/product-domains/src/miniapp/mod.rs`
- Test: inline unit tests in `customization.rs`

- [ ] **Step 1: Add customization metadata types**

Create `customization.rs` with serializable types:

```rust
use crate::miniapp::types::MiniAppPermissions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniAppCustomizationOriginKind {
    Builtin,
    Imported,
    UserCreated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppCustomizationOrigin {
    pub kind: MiniAppCustomizationOriginKind,
    pub builtin_id: Option<String>,
    pub builtin_version: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppAvailableBuiltinUpdate {
    pub builtin_version: u32,
    pub detected_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppCustomizationMetadata {
    pub origin: MiniAppCustomizationOrigin,
    pub local_override: bool,
    pub last_applied_draft_id: Option<String>,
    pub available_builtin_update: Option<MiniAppAvailableBuiltinUpdate>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppPermissionDiff {
    pub high_risk: bool,
    pub added: Vec<String>,
    pub expanded: Vec<String>,
    pub removed: Vec<String>,
}
```

- [ ] **Step 2: Add permission diff helper**

Add a helper that compares active and draft permissions and classifies additions or expansions:

```rust
pub fn diff_permissions(
    active: &MiniAppPermissions,
    draft: &MiniAppPermissions,
) -> MiniAppPermissionDiff {
    let mut added = Vec::new();
    let mut expanded = Vec::new();
    let mut removed = Vec::new();

    diff_string_list(
        "fs.read",
        active.fs.as_ref().and_then(|fs| fs.read.as_ref()),
        draft.fs.as_ref().and_then(|fs| fs.read.as_ref()),
        &mut added,
        &mut expanded,
        &mut removed,
    );
    diff_string_list(
        "fs.write",
        active.fs.as_ref().and_then(|fs| fs.write.as_ref()),
        draft.fs.as_ref().and_then(|fs| fs.write.as_ref()),
        &mut added,
        &mut expanded,
        &mut removed,
    );
    diff_string_list(
        "shell.allow",
        active.shell.as_ref().and_then(|shell| shell.allow.as_ref()),
        draft.shell.as_ref().and_then(|shell| shell.allow.as_ref()),
        &mut added,
        &mut expanded,
        &mut removed,
    );
    diff_string_list(
        "net.allow",
        active.net.as_ref().and_then(|net| net.allow.as_ref()),
        draft.net.as_ref().and_then(|net| net.allow.as_ref()),
        &mut added,
        &mut expanded,
        &mut removed,
    );

    if active.node.as_ref().and_then(|n| n.enabled).unwrap_or(false)
        != draft.node.as_ref().and_then(|n| n.enabled).unwrap_or(false)
    {
        if draft.node.as_ref().and_then(|n| n.enabled).unwrap_or(false) {
            added.push("node.enabled".to_string());
        } else {
            removed.push("node.enabled".to_string());
        }
    }

    if active.ai.as_ref().and_then(|ai| ai.enabled).unwrap_or(false)
        != draft.ai.as_ref().and_then(|ai| ai.enabled).unwrap_or(false)
    {
        if draft.ai.as_ref().and_then(|ai| ai.enabled).unwrap_or(false) {
            added.push("ai.enabled".to_string());
        } else {
            removed.push("ai.enabled".to_string());
        }
    }

    let high_risk = added
        .iter()
        .chain(expanded.iter())
        .any(|item| is_high_risk_permission_change(item));

    MiniAppPermissionDiff {
        high_risk,
        added,
        expanded,
        removed,
    }
}

fn diff_string_list(
    label: &str,
    active: Option<&Vec<String>>,
    draft: Option<&Vec<String>>,
    added: &mut Vec<String>,
    expanded: &mut Vec<String>,
    removed: &mut Vec<String>,
) {
    let active = active.cloned().unwrap_or_default();
    let draft = draft.cloned().unwrap_or_default();
    for value in &draft {
        if !active.contains(value) {
            if active.is_empty() {
                added.push(format!("{label}:{value}"));
            } else {
                expanded.push(format!("{label}:{value}"));
            }
        }
    }
    for value in &active {
        if !draft.contains(value) {
            removed.push(format!("{label}:{value}"));
        }
    }
}

pub fn is_high_risk_permission_change(item: &str) -> bool {
    item.starts_with("fs.write:")
        || item.starts_with("fs.read:")
        || item.starts_with("shell.allow:")
        || item.starts_with("net.allow:")
        || item == "node.enabled"
        || item == "ai.enabled"
}
```

- [ ] **Step 3: Export the module**

Add to `src/crates/product-domains/src/miniapp/mod.rs`:

```rust
pub mod customization;
```

- [ ] **Step 4: Add unit tests**

Cover safe source-only changes and high-risk permission additions:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::miniapp::types::{FsPermissions, MiniAppPermissions, ShellPermissions};

    fn empty_perms() -> MiniAppPermissions {
        MiniAppPermissions {
            fs: None,
            shell: None,
            net: None,
            node: None,
            ai: None,
        }
    }

    #[test]
    fn detects_high_risk_fs_write_addition() {
        let active = empty_perms();
        let mut draft = empty_perms();
        draft.fs = Some(FsPermissions {
            read: None,
            write: Some(vec!["{workspace}".to_string()]),
        });

        let diff = diff_permissions(&active, &draft);

        assert!(diff.high_risk);
        assert!(diff.added.iter().any(|item| item == "fs.write:{workspace}"));
    }

    #[test]
    fn detects_shell_allow_expansion() {
        let mut active = empty_perms();
        active.shell = Some(ShellPermissions {
            allow: Some(vec!["git".to_string()]),
        });
        let mut draft = empty_perms();
        draft.shell = Some(ShellPermissions {
            allow: Some(vec!["git".to_string(), "node".to_string()]),
        });

        let diff = diff_permissions(&active, &draft);

        assert!(diff.high_risk);
        assert!(diff.expanded.iter().any(|item| item == "shell.allow:node"));
    }
}
```

- [ ] **Step 5: Verify product-domain tests**

Run:

```bash
cargo test -p bitfun-product-domains --features miniapp customization
```

Expected: the new customization tests pass.

### Task 2: Add Draft Storage And Local Override Metadata

**Files:**
- Modify: `src/crates/core/src/miniapp/storage.rs`
- Test: existing `storage.rs` test module

- [ ] **Step 1: Add constants and path helpers**

Add constants:

```rust
const DRAFTS_DIR: &str = ".drafts";
const DRAFT_JSON: &str = "draft.json";
const CUSTOMIZATION_JSON: &str = ".customization.json";
```

Add helpers:

```rust
fn drafts_root(&self) -> PathBuf {
    self.path_manager.miniapps_dir().join(DRAFTS_DIR)
}

fn app_drafts_dir(&self, app_id: &str) -> PathBuf {
    self.drafts_root().join(app_id)
}

fn draft_dir(&self, app_id: &str, draft_id: &str) -> PathBuf {
    self.app_drafts_dir(app_id).join(draft_id)
}

fn draft_source_dir(&self, app_id: &str, draft_id: &str) -> PathBuf {
    self.draft_dir(app_id, draft_id).join(SOURCE_DIR)
}

fn customization_path(&self, app_id: &str) -> PathBuf {
    self.app_dir(app_id).join(CUSTOMIZATION_JSON)
}
```

- [ ] **Step 2: Add draft load/save methods**

Implement methods that mirror active app storage but operate on `draft_dir`:

```rust
pub async fn save_draft(
    &self,
    app_id: &str,
    draft_id: &str,
    draft: &MiniApp,
    draft_json: &serde_json::Value,
) -> BitFunResult<()> {
    let dir = self.draft_dir(app_id, draft_id);
    let source = self.draft_source_dir(app_id, draft_id);
    tokio::fs::create_dir_all(&source).await.map_err(|e| {
        BitFunError::io(format!("Failed to create miniapp draft dir: {}", e))
    })?;

    tokio::fs::write(dir.join(DRAFT_JSON), serde_json::to_string_pretty(draft_json)?)
        .await
        .map_err(|e| BitFunError::io(format!("Failed to write draft.json: {}", e)))?;

    let meta = MiniAppMeta::from(draft);
    tokio::fs::write(dir.join(META_JSON), serde_json::to_string_pretty(&meta)?)
        .await
        .map_err(|e| BitFunError::io(format!("Failed to write draft meta.json: {}", e)))?;
    tokio::fs::write(source.join(INDEX_HTML), &draft.source.html).await?;
    tokio::fs::write(source.join(STYLE_CSS), &draft.source.css).await?;
    tokio::fs::write(source.join(UI_JS), &draft.source.ui_js).await?;
    tokio::fs::write(source.join(WORKER_JS), &draft.source.worker_js).await?;
    tokio::fs::write(
        source.join(ESM_DEPS_JSON),
        serde_json::to_string_pretty(&draft.source.esm_dependencies)?,
    )
    .await?;
    tokio::fs::write(dir.join(COMPILED_HTML), &draft.compiled_html).await?;
    Ok(())
}
```

Use the existing `write_package_json` shape by adding a draft-aware package writer or by writing package JSON inline with `build_package_json`.

- [ ] **Step 3: Add draft storage methods**

Add methods:

```rust
pub async fn load_draft_storage(
    &self,
    app_id: &str,
    draft_id: &str,
) -> BitFunResult<serde_json::Value> {
    let p = self.draft_dir(app_id, draft_id).join(STORAGE_JSON);
    if !p.exists() {
        return Ok(serde_json::json!({}));
    }
    let c = tokio::fs::read_to_string(&p).await?;
    Ok(serde_json::from_str(&c).unwrap_or_else(|_| serde_json::json!({})))
}

pub async fn save_draft_storage(
    &self,
    app_id: &str,
    draft_id: &str,
    key: &str,
    value: serde_json::Value,
) -> BitFunResult<()> {
    let dir = self.draft_dir(app_id, draft_id);
    tokio::fs::create_dir_all(&dir).await?;
    let mut current = self.load_draft_storage(app_id, draft_id).await?;
    let obj = current
        .as_object_mut()
        .ok_or_else(|| BitFunError::validation("Draft storage is not an object".to_string()))?;
    obj.insert(key.to_string(), value);
    tokio::fs::write(dir.join(STORAGE_JSON), serde_json::to_string_pretty(&current)?).await?;
    Ok(())
}
```

- [ ] **Step 4: Add customization metadata IO**

Add:

```rust
pub async fn load_customization_metadata(
    &self,
    app_id: &str,
) -> BitFunResult<Option<MiniAppCustomizationMetadata>> {
    let p = self.customization_path(app_id);
    if !p.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(&p).await?;
    Ok(Some(serde_json::from_str(&content)?))
}

pub async fn save_customization_metadata(
    &self,
    app_id: &str,
    metadata: &MiniAppCustomizationMetadata,
) -> BitFunResult<()> {
    tokio::fs::write(
        self.customization_path(app_id),
        serde_json::to_string_pretty(metadata)?,
    )
    .await?;
    Ok(())
}
```

Import `MiniAppCustomizationMetadata` from `bitfun_product_domains::miniapp::customization`.

- [ ] **Step 5: Add storage tests**

Extend `storage.rs` tests to verify:

- `list_app_ids` ignores `.drafts`.
- Draft storage write does not create a visible app id.
- Active storage and draft storage are independent.

- [ ] **Step 6: Verify storage tests**

Run:

```bash
cargo test -p bitfun-core miniapp::storage -- --nocapture
```

Expected: storage tests pass.

### Task 3: Add Manager Draft Lifecycle

**Files:**
- Modify: `src/crates/core/src/miniapp/manager.rs`
- Modify: `src/crates/core/src/miniapp/mod.rs`
- Test: `manager.rs` test module or a new `src/crates/core/src/miniapp/customization_tests.rs`

- [ ] **Step 1: Add draft DTOs**

Add Rust structs near `MiniAppManager` or in a focused module if the file becomes too large:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MiniAppDraft {
    pub app_id: String,
    pub draft_id: String,
    pub source_version: u32,
    pub created_at: i64,
    pub updated_at: i64,
    pub app: MiniApp,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MiniAppDraftManifest {
    pub app_id: String,
    pub draft_id: String,
    pub source_version: u32,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}
```

- [ ] **Step 2: Implement `create_draft`**

Implementation rules:

- Load active app.
- Create `draft_id` using UUID.
- Keep logical app id as the active app id in metadata.
- Compile draft source with draft run scope support from Task 5.
- Save under `.drafts`.
- Initialize draft `storage.json` as `{}`.

Signature:

```rust
pub async fn create_draft(
    &self,
    app_id: &str,
    theme: &str,
    workspace_root: Option<&Path>,
) -> BitFunResult<MiniAppDraft>
```

- [ ] **Step 3: Implement `sync_draft_from_fs`**

Reload draft source files from draft `source/`, increment draft metadata timestamp, recompile draft preview, and save draft without touching the active app.

Signature:

```rust
pub async fn sync_draft_from_fs(
    &self,
    app_id: &str,
    draft_id: &str,
    theme: &str,
    workspace_root: Option<&Path>,
) -> BitFunResult<MiniAppDraft>
```

- [ ] **Step 4: Implement `permission_diff_for_draft`**

Load active app and draft app, then call `diff_permissions`.

Signature:

```rust
pub async fn permission_diff_for_draft(
    &self,
    app_id: &str,
    draft_id: &str,
) -> BitFunResult<MiniAppPermissionDiff>
```

- [ ] **Step 5: Implement `set_draft_permissions`**

Load the draft app, replace its permissions, recompile the draft preview, and save the draft without touching the active app.

Signature:

```rust
pub async fn set_draft_permissions(
    &self,
    app_id: &str,
    draft_id: &str,
    permissions: MiniAppPermissions,
    theme: &str,
    workspace_root: Option<&Path>,
) -> BitFunResult<MiniAppDraft>
```

- [ ] **Step 6: Implement draft storage methods on manager**

Forward to storage draft methods:

```rust
pub async fn get_draft_storage(
    &self,
    app_id: &str,
    draft_id: &str,
    key: &str,
) -> BitFunResult<serde_json::Value>

pub async fn set_draft_storage(
    &self,
    app_id: &str,
    draft_id: &str,
    key: &str,
    value: serde_json::Value,
) -> BitFunResult<()>
```

- [ ] **Step 7: Implement `apply_draft`**

Apply order must match the design document:

```rust
pub async fn apply_draft(
    &self,
    app_id: &str,
    draft_id: &str,
    theme: &str,
    workspace_root: Option<&Path>,
) -> BitFunResult<MiniApp>
```

Required behavior:

- Load current active app.
- Load draft app.
- Save active version snapshot before writing.
- Copy draft source, metadata fields, permissions, and AI context into active app.
- Preserve active `storage.json`.
- Increment active version and updated timestamp.
- Build runtime state from draft source.
- Save active app.
- Mark local override if app id is a built-in id or if existing customization metadata origin is built-in.
- Return updated active app.

- [ ] **Step 8: Implement `discard_draft`**

Remove `.drafts/{app_id}/{draft_id}` and return success if the directory is already gone.

Signature:

```rust
pub async fn discard_draft(&self, app_id: &str, draft_id: &str) -> BitFunResult<()>
```

- [ ] **Step 9: Add manager tests**

Add tests for:

- Create draft leaves active version unchanged.
- Sync draft after editing draft source changes draft compiled HTML only.
- Apply draft creates a rollback version and updates active source.
- Active storage survives apply.
- Discard draft leaves active source unchanged.
- Draft storage writes never change active storage.
- Draft permission update changes only draft permissions until apply.

- [ ] **Step 10: Verify manager tests**

Run:

```bash
cargo test -p bitfun-core miniapp -- --nocapture
```

Expected: MiniApp manager/storage tests pass.

### Task 4: Protect Customized Built-in Apps During Reseed

**Files:**
- Modify: `src/crates/core/src/miniapp/builtin/mod.rs`
- Test: new or existing built-in seeding tests

- [ ] **Step 1: Check local override before overwriting source**

In `seed_one`, after reading marker and before writing `meta.json` and `source/`, load customization metadata. If `local_override` is true:

- Do not write `meta.json`.
- Do not write `source/*`.
- Do not write `package.json`.
- Do not write `compiled.html`.
- Save customization metadata with `available_builtin_update`.
- Write or update `.builtin-version` only if needed to avoid repeated update detection loops.

- [ ] **Step 2: Keep non-customized reseed behavior unchanged**

If no local override exists, the existing reseed path should still overwrite source and preserve `storage.json`.

- [ ] **Step 3: Add tests**

Test both cases:

- Non-customized built-in app receives newer source when bundled version increases.
- Customized built-in app keeps active source and records available official update.

- [ ] **Step 4: Verify built-in tests**

Run:

```bash
cargo test -p bitfun-core seed_builtin_miniapps -- --nocapture
```

Expected: customized built-in source is preserved.

### Task 5: Add Desktop Commands And TypeScript API Wrappers

**Files:**
- Modify: `src/apps/desktop/src/api/miniapp_api.rs`
- Modify: command registration file under `src/apps/desktop/src`
- Modify: `src/web-ui/src/infrastructure/api/service-api/MiniAppAPI.ts`

- [ ] **Step 1: Add Tauri command request/response structs**

Add request structs:

```rust
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppDraftRequest {
    pub app_id: String,
    pub draft_id: Option<String>,
    pub theme: Option<String>,
    pub workspace_path: Option<String>,
}
```

- [ ] **Step 2: Add commands**

Add commands:

- `create_miniapp_draft`
- `get_miniapp_draft`
- `miniapp_draft_sync_from_fs`
- `miniapp_draft_set_permissions`
- `get_miniapp_draft_permission_diff`
- `get_miniapp_draft_storage`
- `set_miniapp_draft_storage`
- `miniapp_draft_host_call`
- `miniapp_draft_worker_call`
- `miniapp_draft_worker_stop`
- `apply_miniapp_draft`
- `discard_miniapp_draft`

Each command should use `workspace_root_from_input` like existing MiniApp commands.

- [ ] **Step 3: Emit events**

Emit:

- `miniapp-draft-created`
- `miniapp-draft-updated`
- `miniapp-draft-applied`
- `miniapp-draft-discarded`
- Existing `miniapp-updated` after apply.

- [ ] **Step 4: Add TypeScript API wrappers**

In `MiniAppAPI.ts`, add:

```ts
export interface MiniAppDraft {
  appId: string;
  draftId: string;
  sourceVersion: number;
  createdAt: number;
  updatedAt: number;
  app: MiniApp;
}

export interface MiniAppPermissionDiff {
  highRisk: boolean;
  added: string[];
  expanded: string[];
  removed: string[];
}
```

Add wrapper methods:

- `createDraft(appId, theme, workspacePath)`
- `getDraft(appId, draftId, theme, workspacePath)`
- `syncDraftFromFs(appId, draftId, theme, workspacePath)`
- `setDraftPermissions(appId, draftId, permissions, theme, workspacePath)`
- `getDraftPermissionDiff(appId, draftId)`
- `getDraftStorage(appId, draftId, key)`
- `setDraftStorage(appId, draftId, key, value)`
- `draftHostCall(appId, draftId, method, params, workspacePath)`
- `draftWorkerCall(appId, draftId, method, params, workspacePath)`
- `draftWorkerStop(appId, draftId)`
- `applyDraft(appId, draftId, theme, workspacePath)`
- `discardDraft(appId, draftId)`

- [ ] **Step 5: Run PR 1 verification**

Run:

```bash
cargo test -p bitfun-product-domains --features miniapp customization
cargo test -p bitfun-core miniapp -- --nocapture
cargo check --workspace
pnpm run type-check:web
```

Expected: all commands pass. If full workspace cargo tests are too slow, run them before PR publish.

## PR 2: Host Customization Entry And Assistant Workflow

Risk: Medium to High. This adds a new product workflow, but depends on PR 1 APIs and avoids new agent runtime primitives by reusing existing side-thread chat.

### Task 6: Add Frontend Customization Types And Prompt Builder

**Files:**
- Create: `src/web-ui/src/app/scenes/miniapps/customization/miniAppCustomizationTypes.ts`
- Create: `src/web-ui/src/app/scenes/miniapps/customization/miniAppCustomizationPrompt.ts`
- Test: `src/web-ui/src/app/scenes/miniapps/customization/miniAppCustomizationPrompt.test.ts`

- [ ] **Step 1: Add UI state types**

```ts
import type { MiniAppDraft, MiniAppPermissionDiff } from '@/infrastructure/api/service-api/MiniAppAPI';

export type MiniAppCustomizationStage =
  | 'idle'
  | 'notice'
  | 'drafting'
  | 'preview'
  | 'permission-review'
  | 'applying';

export interface MiniAppCustomizationState {
  stage: MiniAppCustomizationStage;
  draft: MiniAppDraft | null;
  permissionDiff: MiniAppPermissionDiff | null;
  assistantSessionId: string | null;
  error: string | null;
}
```

- [ ] **Step 2: Add prompt builder**

```ts
export function buildMiniAppCustomizationPrompt(params: {
  appId: string;
  appName: string;
  draftId: string;
  draftRoot: string;
  userRequest: string;
}): string {
  return [
    `You are customizing a BitFun MiniApp draft.`,
    `App: ${params.appName} (${params.appId})`,
    `Draft id: ${params.draftId}`,
    `Draft root: ${params.draftRoot}`,
    ``,
    `Edit only files under the draft root.`,
    `Do not edit the active app directory.`,
    `Do not add permissions unless the user request truly needs them.`,
    `If new fs, shell, net, node, npm, or ai permissions are needed, explain why before changing them.`,
    ``,
    `User request:`,
    params.userRequest,
  ].join('\n');
}
```

- [ ] **Step 3: Add tests**

Verify the prompt includes the draft root and active-app warning.

- [ ] **Step 4: Run prompt tests**

Run:

```bash
pnpm --dir src/web-ui run test:run -- miniAppCustomizationPrompt
```

Expected: prompt builder tests pass.

### Task 7: Add Host Trigger And Risk Notice

**Files:**
- Create: `src/web-ui/src/app/scenes/miniapps/customization/MiniAppCustomizeEntry.tsx`
- Create: `src/web-ui/src/app/scenes/miniapps/customization/useMiniAppCustomizeHotspot.ts`
- Modify: `src/web-ui/src/app/scenes/miniapps/MiniAppScene.tsx`
- Modify: `src/web-ui/src/app/scenes/miniapps/MiniAppScene.scss`
- Test: component tests if MiniApp scene test harness exists; otherwise hook tests for shortcut behavior

- [ ] **Step 1: Implement trigger component**

Use lucide `WandSparkles` or closest existing icon. The button should be in the host scene chrome, not inside iframe.

Props:

```ts
interface MiniAppCustomizeEntryProps {
  disabled?: boolean;
  onOpen: () => void;
}
```

- [ ] **Step 2: Implement shortcut/hotspot hook**

The hook should:

- Listen only while MiniApp scene is mounted.
- Ignore keydown from editable inputs.
- Open on `Ctrl+Shift+E` on Windows/Linux and `Meta+Shift+E` on macOS.
- Reveal hotspot button after hover near the configured corner for a short delay.

- [ ] **Step 3: Wire into MiniAppScene**

Add local state:

```ts
const [customizeOpen, setCustomizeOpen] = useState(false);
```

Render the entry alongside the reload button and open the panel from the trigger.

- [ ] **Step 4: Add risk notice copy**

The notice must include:

- Draft copy is edited first.
- Active app is unchanged until Apply.
- Current version is saved before Apply.
- New permissions require separate confirmation.

- [ ] **Step 5: Verify UI compile**

Run:

```bash
pnpm run type-check:web
```

Expected: no TypeScript errors.

### Task 8: Launch Assistant Side Thread For Draft Editing

**Files:**
- Create: `src/web-ui/src/app/scenes/miniapps/customization/MiniAppCustomizePanel.tsx`
- Modify: `src/web-ui/src/app/scenes/miniapps/MiniAppScene.tsx`
- Use existing: `src/web-ui/src/flow_chat/services/BtwThreadService.ts`
- Use existing: `src/web-ui/src/flow_chat/services/openBtwSession.ts`

- [ ] **Step 1: Create draft before assistant launch**

When the user submits a customization request:

1. Call `miniAppAPI.createDraft(app.id, themeType, workspacePath)`.
2. Store the returned draft.
3. Build the assistant prompt with `buildMiniAppCustomizationPrompt`.

- [ ] **Step 2: Create side-thread session**

Reuse the existing BTW child-session path rather than adding a new session kind in the first version.

The child session name should be localized as "Customize MiniApp: {name}".

- [ ] **Step 3: Send the initial prompt**

Use `FlowChatManager.getInstance().sendMessage(...)` with the generated prompt and a user-facing display message equal to the user's original request.

- [ ] **Step 4: Open the side thread in the auxiliary pane**

Call `openBtwSessionInAuxPane` with the child session id and current parent session id when available. If no parent session exists, create or reuse a normal agentic session first using existing flow chat creation behavior.

- [ ] **Step 5: Add failure handling**

If session creation or send fails after a draft was created, keep the draft and show "Draft created, assistant launch failed" with buttons for retry and discard.

### Task 9: Add Draft Preview And Refresh

**Files:**
- Create: `src/web-ui/src/app/scenes/miniapps/customization/MiniAppDraftPreview.tsx`
- Modify: `src/web-ui/src/app/scenes/miniapps/components/MiniAppRunner.tsx`
- Modify: `src/web-ui/src/app/scenes/miniapps/hooks/useMiniAppBridge.ts`
- Modify: `src/web-ui/src/app/scenes/miniapps/customization/MiniAppCustomizePanel.tsx`

- [ ] **Step 1: Add run scope type**

In `MiniAppAPI.ts` or a MiniApp scene type file:

```ts
export type MiniAppRunScope =
  | { kind: 'active'; appId: string }
  | { kind: 'draft'; appId: string; draftId: string };
```

- [ ] **Step 2: Update MiniAppRunner**

Add optional prop:

```ts
runScope?: MiniAppRunScope;
```

Default to `{ kind: 'active', appId: app.id }`.

- [ ] **Step 3: Update bridge routing**

In `useMiniAppBridge`, route `storage.get`, `storage.set`, `workerCall`, and `hostCall` using `runScope`. Draft scope must call the draft-aware API endpoints added in PR 1.

- [ ] **Step 4: Add draft preview component**

Render the draft app with:

```tsx
<MiniAppRunner
  app={draft.app}
  runScope={{ kind: 'draft', appId: draft.appId, draftId: draft.draftId }}
/>
```

- [ ] **Step 5: Add refresh**

The refresh button calls `miniAppAPI.syncDraftFromFs(...)` and updates preview HTML. Use this after assistant edits files.

### Task 10: Add Permission Diff And Apply/Discard Controls

**Files:**
- Create: `src/web-ui/src/app/scenes/miniapps/customization/MiniAppPermissionDiffDialog.tsx`
- Modify: `src/web-ui/src/app/scenes/miniapps/customization/MiniAppCustomizePanel.tsx`
- Modify: `src/web-ui/src/app/scenes/miniapps/MiniAppScene.tsx`
- Modify: `src/web-ui/src/app/scenes/miniapps/hooks/useMiniAppCatalogSync.ts`

- [ ] **Step 1: Check permission diff before apply**

When Apply is clicked:

1. Call `miniAppAPI.getDraftPermissionDiff(app.id, draft.draftId)`.
2. If `highRisk` is false, continue to apply confirmation.
3. If `highRisk` is true, show `MiniAppPermissionDiffDialog`.

- [ ] **Step 2: Implement high-risk confirmation**

The dialog lists `added`, `expanded`, and `removed`. The high-risk button text should be explicit, such as "Apply and grant permissions".

- [ ] **Step 3: Apply draft**

Call `miniAppAPI.applyDraft(app.id, draft.draftId, themeType, workspacePath)`.

After success:

- Update current app state with returned active app.
- Increment MiniAppRunner key to reload iframe.
- Close draft preview.
- Refresh gallery store through existing `miniapp-updated` event.

- [ ] **Step 4: Discard draft**

Call `miniAppAPI.discardDraft(app.id, draft.draftId)` and close customization state. Do not close the active app scene.

- [ ] **Step 5: Keep rollback visible**

Do not create a new rollback UI if existing version/rollback surfaces already cover MiniApps. If no user-facing rollback exists, show a small message after Apply: "Previous version saved; use MiniApp versions to roll back."

### Task 11: Locales, Tests, And Verification

**Files:**
- Modify: `src/web-ui/src/locales/en-US/scenes/miniapp.json`
- Modify: `src/web-ui/src/locales/zh-CN/scenes/miniapp.json`
- Modify: `src/web-ui/src/locales/zh-TW/scenes/miniapp.json`
- Add tests next to new customization modules

- [ ] **Step 1: Add locale keys**

Add keys for:

- Customize trigger tooltip.
- Risk notice title/body.
- Prompt input placeholder.
- Start customization.
- Refresh preview.
- Apply draft.
- Apply and grant permissions.
- Discard draft.
- Draft launch failure.
- Permission diff sections.

- [ ] **Step 2: Add frontend tests**

Cover:

- Prompt builder includes draft root and warning.
- Shortcut/hotspot opens customization.
- High-risk permission diff requires second confirmation.
- Discard leaves active app scene mounted.

- [ ] **Step 3: Run web verification**

Run:

```bash
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
```

Expected: all web checks pass.

- [ ] **Step 4: Run backend smoke after UI integration**

Run:

```bash
cargo check --workspace
cargo test -p bitfun-core miniapp -- --nocapture
```

Expected: backend remains compatible after frontend API integration.

## PR 1 Acceptance Criteria

- Draft APIs exist and are registered.
- Draft source and storage are isolated from active app files.
- Applying a draft snapshots the active app first.
- Discarding a draft leaves active source and storage untouched.
- Customized built-in apps are not overwritten by built-in reseed.
- Permission diff marks high-risk additions.
- Verification commands from PR 1 pass.

## PR 2 Acceptance Criteria

- Every MiniApp scene has the same host-level customization entry.
- Shortcut and hotspot open the same customization flow.
- Risk notice appears before assistant editing starts.
- Assistant edits are directed to draft files only.
- Draft preview runs before Apply.
- High-risk permission diff gets separate confirmation.
- Apply updates the current app and preserves rollback.
- Discard removes draft state without changing active app.
- Web and MiniApp backend smoke checks pass.

## Explicit Deferrals

- Official update merge UI for customized built-in apps.
- Marketplace, sharing, or cloud sync.
- Embedded full chat implementation inside the MiniApp scene.
- Automatic permission reasoning by the frontend without backend diff data.
- Data migration from draft storage to active storage.
- New `app.bitfun.*` or internal BitFun service APIs exposed to MiniApps.
