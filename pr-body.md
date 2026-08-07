## Summary

Fixes #2035

Adds the ability to delete remote server entries from the ACP Agents configuration page. Previously, when a remote SSH server was used, an ACP Agent entry was auto-created but could not be removed. If the server was no longer in use, the entry cluttered the UI indefinitely.

## Changes

- **AcpAgentsConfig.tsx**: Added a `deleteRemoteConnection` handler that calls `sshApi.deleteConnection()`, removes the connection from `savedConnections` state, cleans up probe data, and shows success/error notifications. Added a Delete button (red/danger variant) next to the existing Refresh button for each remote server row.
- **Locale files** (`en-US`, `zh-CN`, `zh-TW`): Added `remote.deleteConnection`, `remote.deleteConfirm`, `notifications.deleteConnectionSuccess`, and `notifications.deleteConnectionFailed` keys.

## Behavior

1. User clicks the Delete button on a remote server row
2. A confirmation dialog asks: "Remove the remote server \"{{name}}\" from saved SSH connections? Its ACP agent entries will also be removed."
3. On confirm, `sshApi.deleteConnection(connectionId)` is called
4. The connection is removed from the saved connections list and probe data is cleaned up
5. A success notification is shown (or error notification on failure)

## Validation

- TypeScript: `tsc --noEmit` passes (no errors in modified files)
- Tests: All 7 existing `AcpAgentsConfig.test.tsx` tests pass
- i18n audit: `pnpm run i18n:audit` passes with 0 warnings
- Locale key parity verified across en-US, zh-CN, zh-TW
