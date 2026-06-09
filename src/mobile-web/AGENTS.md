# AGENTS.md

Mobile web is the browser-based remote control client for BitFun desktop sessions.

## Boundaries

- Keep mobile-web logic inside `src/mobile-web`; do not import from `src/web-ui`.
- Treat pairing, reconnect, disconnect, session list, and chat state as one connected product flow.
- Keep connection state semantics consistent across persistent indicators, banners, dialogs, and disabled states.
- User-facing strings should use the mobile-web i18n message system when one is already present for the surface being changed.
- Locale ids and aliases come from `src/shared/i18n/contract/locales.json`
  through generated files. Do not import Web UI locale resources to reuse copy.
- Do not commit local pairing URLs, user IDs, logs, screenshots with sensitive data, or temporary AI prompts.

## Where to look first

| Area | Paths |
|---|---|
| Pairing | `src/pages/PairingPage.tsx`, `src/services/RelayHttpClient.ts` |
| Session list | `src/pages/SessionListPage.tsx`, `src/services/store.ts` |
| Chat | `src/pages/ChatPage.tsx`, `src/services/RemoteSessionManager.ts` |
| Connection health / reconnect | `src/App.tsx`, `src/services/RemoteSessionManager.ts`, `src/services/store.ts` |
| Styles | `src/styles/`, `src/theme/` |
| Messages | `src/i18n/messages.ts` |

## Verification

Run the focused mobile-web checks after changes:

```bash
pnpm --dir src/mobile-web run type-check
pnpm run build:mobile-web
```

For pairing, reconnect, disconnect, or chat behavior changes, also describe manual verification in the PR, including the browser/device used and the observed state transitions.
