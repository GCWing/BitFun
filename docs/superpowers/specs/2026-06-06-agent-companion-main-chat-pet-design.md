# Agent Companion Pet in Main Chat — Design

## Goal

Render the Agent Companion Pet in the bottom-left corner of the main chat message
canvas when `agent_companion_display_mode === 'input'`, so the pet is visible
in the regular chat view (not only in the floating ToolbarMode window).

## Background

Currently `input` mode renders the pet only in
`src/web-ui/src/flow_chat/components/toolbar-mode/ToolbarMode.tsx:618-626`,
which is a separate collapsed-window mode the user must enter explicitly via
the nav panel. The regular chat view's `ChatInput.tsx` has no pet render
entry. The setting description
`src/web-ui/src/locales/zh-CN/settings/session-config.json:23`
("在会话输入框收起时显示") is misleading.

This change makes the pet visible in the chat canvas while the user is in the
normal chat view.

## Scope

In scope:

- Render the pet in `ModernFlowChatContainer` when `input` mode is active.
- Update the Chinese (zh-CN) and English (en-US) setting description.
- No changes to `desktop` mode, `ToolbarMode`, settings logic, or Rust.

Out of scope:

- Pet drag/resize/reposition (the pet is fixed at the bottom-left).
- New mood states (reuses existing rest / working / waiting).
- User-imported pet packaging changes.

## Design

### Render point

`src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx:898`
(the last child of the `modern-flowchat-container__messages` div, before the
closing `</div>` of the outer container). The pet is a sibling of
`VirtualMessageList` / `WelcomePanel` / `HistorySessionPlaceholder`, not a
child of any of them.

### Visibility

`showCompanionPet` is true when all three are satisfied:

- `enable_agent_companion` is true
- `agent_companion_display_mode === 'input'`
- `agent_companion_pet` is a non-null selection

Logic mirrors `src/web-ui/src/flow_chat/components/toolbar-mode/ToolbarMode.tsx:78-83`.

### Mood

`companionMood` reuses the logic from
`src/web-ui/src/flow_chat/components/toolbar-mode/ToolbarMode.tsx:72-76`:

- `rest` when not processing
- `waiting` when a tool is currently running
- `working` otherwise (processing, no active tool)

`ModernFlowChatContainer` already exposes streaming/processing state through
its existing hooks (`derivedState`, `toolbarState`, `currentStreamState`).
The mood computation reads the same sources the chat body uses; no new
context or store is required.

### Settings subscription

`ModernFlowChatContainer` subscribes to `aiExperienceConfigService` so the
pet reacts when the user toggles `enable_agent_companion` or changes
`agent_companion_display_mode` while the chat is open. This mirrors
`App.tsx`'s existing listener pattern.

Subscription lifecycle:

- On mount: `ensureConfigWatcher` is already invoked by the service, but the
  container calls `addChangeListener` for an explicit subscription.
- On unmount: the returned unsubscribe function runs in the effect cleanup.

### Layout

- Position: `absolute` inside `modern-flowchat-container__messages`
  (which has `position: relative` set on its parent
  `modern-flowchat-container`).
- Offsets: `left: 16px; bottom: 16px;`
- Size: 48×48 px (matches ToolbarMode).
- `z-index: 50;` — higher than the WelcomePanel, the ChatInput, and the
  virtual list scrollbars. Pointer events stay enabled only on the pet
  element itself; the wrapper is `pointer-events: none` to avoid blocking
  clicks on the message list.
- `cursor: default;` — no click affordance.

### Files changed

1. `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx`
   - Add subscription to `aiExperienceConfigService`.
   - Compute `companionSettings`, `showCompanionPet`, `companionMood`.
   - Render the pet block as the last child of the messages div.
2. `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss`
   - Add the absolutely positioned wrapper rules.
3. `src/web-ui/src/locales/zh-CN/settings/session-config.json`
   - Update `displayInputDesc` to "在主聊天界面左下角显示".
4. `src/web-ui/src/locales/zh-TW/settings/session-config.json`
   - Update the parallel string for Traditional Chinese.
5. `src/web-ui/src/locales/en-US/settings/session-config.json`
   - Update the parallel English string to "Show at the bottom-left of the
     main chat canvas."

## Risks

- **Subscription leak**: the listener must be removed on unmount; tests
  mount/unmount the container so an unsubscribe is required.
- **Stale settings**: the pet is part of the main webview, so the
  service-instance cache is shared with the settings UI. The subscription
  plus a fresh `getSettingsAsync()` on mount avoids stale reads.
- **i18n audit**: changing the resource string requires running
  `pnpm run i18n:audit` to keep key/placeholder parity.

## Verification

- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run lint`
- `pnpm run i18n:audit`
- Focused tests: `pnpm --dir src/web-ui run test:run src/flow_chat/components/modern`

Manual:

- Set `agent_companion_display_mode` to "输入框内" and confirm the pet
  appears at the bottom-left of the chat canvas.
- Start an AI turn and confirm the pet switches to `working` / `waiting`
  mood, then back to `rest` when the turn completes.
- Switch to "悬浮桌面宠物" and confirm the canvas pet disappears while
  the floating window pet appears.
- Switch to the ToolbarMode and confirm only the ToolbarMode pet is
  visible (the canvas pet is gone, because the canvas isn't rendered).
