# AGENTS.md

Mobile web is the browser-based remote control client for OpenBitFun desktop sessions.

## Boundaries

- Keep mobile-web logic inside `src/mobile-web`; do not import from `src/web-ui`.
- Treat pairing, reconnect, disconnect, session list, and chat state as one connected product flow.
- Keep connection state semantics consistent across persistent indicators, banners, dialogs, and disabled states.
- User-facing strings should use the mobile-web i18n message system when one is already present for the surface being changed.
- Locale ids and aliases come from `src/shared/i18n/contract/locales.json`
  through generated files. Do not import Web UI locale resources to reuse copy.
- Do not commit local pairing URLs, user IDs, logs, screenshots with sensitive data, or temporary AI prompts.

## Component style integration

- Load `styles/reset.scss` before shared component CSS. Keep universal resets
  and generic focus defaults in `openbitfun.reset`, below `openbitfun.components`;
  unlayered resets erase shared spacing and focus behavior.
- Adapt shared components through their public `data-openbitfun-part` hooks or
  explicit app classes, not old native tags, positional children, or generated
  CSS-module class names. Recheck selectors whenever component anatomy changes.
- `MobileTextField.className` styles the field surface; `inputClassName` is only
  for editor-specific behavior or typography. Keep borders/background/focus on
  one surface and use the leading/trailing slots for icons.
- Check compact, intermediate/foldable, and wide layouts, including expanded
  controls and keyboard focus. A passing source contract test is not visual QA.

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
pnpm --dir src/mobile-web run test:ui-components
pnpm --dir src/mobile-web run test:account-login # account login without an online desktop
pnpm --dir src/mobile-web run type-check
pnpm run build:mobile-web
```

The build skips work when `src/mobile-web/dist` is newer than every input. Use
`OPENBITFUN_MOBILE_WEB_FORCE_BUILD=1` or `node scripts/mobile-web-build.cjs --force`
only when a rebuild is required despite unchanged inputs.

For pairing, reconnect, disconnect, or chat behavior changes, also describe manual verification in the PR, including the browser/device used and the observed state transitions.
