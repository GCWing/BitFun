# CLI Account Sync & Status Panel Design

## Goal

Align CLI `/login` with Desktop Account Login after authentication:

1. After credentials succeed, if cloud settings exist → choose **Use local** / **Use cloud**.
2. Then run the same auto-sync flow (settings + session upload).
3. If already logged in, `/login` opens an **account status** page (not the credential form) with account info and sync progress.

## Views

| Mode | When | UI |
|---|---|---|
| Login | Not logged in | Existing Auth Server / Username / Password / Login |
| SyncChoice | Login succeeded and `has_cloud_settings` | Use local / Use cloud / Cancel (logout) |
| Account | Logged in (or after sync choice / first login) | User id, relay, this device, devices list, sync status/progress, Logout |

## Sync semantics (match Desktop)

- No cloud settings → treat as first login: upload local config (`is_first_login=true`), then export sessions.
- Use local → `is_first_login=true` (upload local settings).
- Use cloud → `is_first_login=false` (download + import settings).
- Cancel on SyncChoice → logout, return to Login.
- Progress phases: uploading/downloading/applying settings → listing/exporting sessions → done/failed.
- Sync runs in background; Account page reads shared progress state (Esc closes panel, sync continues).

## Implementation

- `account.rs`: structured `LoginResult` + `fetch_settings` for `has_cloud_settings`; `run_auto_sync` using `AccountClient`, `ConfigService`, `PersistenceManager`, `sync_state`.
- `ui/account_panel.rs` (replaces single-purpose login form): multi-mode TUI.
- `/login` entry: if logged in → Account; else → Login.

## Non-goals (this change)

- Continuous debounce push like Desktop `init_auto_sync` (follow-up).
- Peer Device Mode controller entry from CLI Account page.
