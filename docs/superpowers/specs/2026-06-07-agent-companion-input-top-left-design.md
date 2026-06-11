# Agent Companion Pet at ChatInput Top-Left — Design

## Goal

Move the Agent Companion Pet's "input" display mode render from the bottom-left
of the main chat message canvas (currently in `ModernFlowChatContainer`) to the
top-left corner of the chat input box (`ChatInput`'s `__box`), so the pet
visually hangs at the top-left of the input field and the i18n description
matches the actual render point.

## Background

The previous change (commit `c44001f5` and its plan/spec dated 2026-06-06) made
the pet visible in the main chat canvas at the bottom-left corner of
`__messages`. The new direction fixes the pet to the top-left of the actual
input box, which is what the original zh-CN description
"在会话输入框收起时显示" was hinting at.

The pet is 48×48 px, larger than the capsule input box's 44 px height, so the
pet must be allowed to overflow the rounded box bounds vertically (and
horizontally). The `__box` already has `position: relative` and the default
`overflow: visible`, so the pet can sit at `top: 0; left: 0` and overhang.

The `desktop` display mode is unchanged: it still opens a separate Tauri
window via `show_agent_companion_desktop_pet`. Only the `input` mode render
point changes.

## Scope

In scope:

- Render the pet inside `ChatInput`'s `__box` as the first child, at
  `top: 0; left: 0`, 48×48 px, `pointer-events: none` on the wrapper.
- Subscribe `ChatInput` to `aiExperienceConfigService` with a change listener.
- Compute `companionMood` from the active session's last turn (reuse existing
  logic, identical to the previous `ModernFlowChatContainer` derivation).
- Remove the `input`-mode pet render from `ModernFlowChatContainer`
  (component, SCSS, test).
- Update the `displayInputDesc` locale string in zh-CN / zh-TW / en-US.
- Add a `ChatInput.pet.test.tsx` covering show/hide paths and a
  `ModernFlowChatContainer.pet.test.tsx` regression test asserting the pet is
  no longer rendered in `ModernFlowChatContainer`.

Out of scope:

- Pet drag / resize / reposition.
- New mood states (reuses `rest` / `working` / `waiting`).
- `desktop` mode behavior, `ToolbarMode` behavior, settings logic, Rust.
- Capsule-mode size adaptation: pet stays 48×48 in both modes and is allowed
  to overflow the capsule (44 px) vertically by 2 px on each side.

## Design

### Render point

`src/web-ui/src/flow_chat/components/ChatInput.tsx` — first child inside
`bitfun-chat-input__box` (before `__target-switcher`, `__input-area`, and
`__actions`). Rendered as a sibling of the existing internal sections of the
box, not inside `__input-area`, so the pet does not affect the input area's
min-height / padding calculations.

### Wrapper

A new div `bitfun-chat-input__companion-pet` wraps the `<ChatInputPixelPet>`
component. The wrapper has:

- `position: absolute`
- `top: 0; left: 0`
- `z-index: 10` (above the placeholder text and the agent-boost capsule in
  `__actions` — those are at default stacking)
- `width: 48px; height: 48px`
- `pointer-events: none` (clicks pass through to the text editor underneath)
- `cursor: default`

The pet's own element re-enables `pointer-events: auto` to preserve the
existing pet behavior (no click affordance, but no accidental block on hover
animations either).

### Pet position in different box states

| `__box` state | Pet position | Visual result |
|---|---|---|
| `--capsule` (44 px tall) | `top: 0; left: 0`, 48×48 | Overhangs 2 px above and 2 px below the capsule, 2 px to the left of the left border-radius |
| `--multi-line` (≥ 76 px tall) | `top: 0; left: 0`, 48×48 | Sits at the top-left of the box, just inside the top border; overhangs 2 px to the left of the left border-radius |

The `__box` has no explicit `overflow` rule, so the default `visible`
applies — overflow renders correctly. The parent `ContextDropZone` does not
clip its children in the input area.

### Visibility

`showCompanionPet` is true when all three are satisfied:

- `enable_agent_companion` is true
- `agent_companion_display_mode === 'input'`
- `agent_companion_pet` is a non-null selection

Same predicate as the previous `ModernFlowChatContainer` implementation
(parallel to `ToolbarMode.tsx:78-83`).

### Settings subscription

`ChatInput` subscribes to `aiExperienceConfigService` so the pet reacts when
the user toggles `enable_agent_companion` or switches
`agent_companion_display_mode` while the chat is open. Subscription lifecycle:

- On mount: initial value via `getSettings()` (synchronous); async refresh
  via `getSettingsAsync()` to pick up persisted settings.
- On change: `addChangeListener` updates local `companionSettings` state.
- On unmount: returned unsubscribe function runs in the effect cleanup.

This fixes the stale-settings issue noted in the project's `KNOWN_ISSUES`
for the sibling `ToolbarMode` (which only loads on mount).

### Mood

`companionMood` reuses the same logic as the previous
`ModernFlowChatContainer` implementation: read the active session via the
existing `useActiveSessionState` hook and inspect the last turn:

- `rest` when not processing
- `waiting` when a tool is currently running
- `working` otherwise (processing, no active tool)

The `ChatInput` already uses `useActiveSessionState`, so no new hook or
context is required.

### Removal of the previous render point

`ModernFlowChatContainer` no longer needs:

- The `companionSettings` state and the `addChangeListener` subscription.
- The `showCompanionPet` and `companionMood` derivations.
- The pet render block at the end of the `__messages` div.
- The `aiExperienceConfigService` and `ChatInputPixelPet` imports (unless
  other call sites in the same file need them — current state shows they
  are used only for the pet).

### Files changed

1. `src/web-ui/src/flow_chat/components/ChatInput.tsx`
   - Add subscription to `aiExperienceConfigService`.
   - Compute `companionSettings`, `showCompanionPet`, `companionMood`.
   - Render the pet block as the first child of `__box`.
2. `src/web-ui/src/flow_chat/components/ChatInput.scss`
   - Add `&__companion-pet` absolutely-positioned wrapper rules.
3. `src/web-ui/src/flow_chat/components/ChatInput.pet.test.tsx` (new)
   - Verify pet shows/hides on settings change, on enable toggle, on display
     mode toggle, and the `data-testid="chat-input-pet"` is inside the
     `__box`.
4. `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx`
   - Remove the subscription, derivations, render block, and unused
     imports.
5. `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss`
   - Remove the `&__companion-pet` and `&__companion-pet-element` rules.
6. `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx`
   - Rewrite the existing tests to assert the pet is NOT rendered inside
     `ModernFlowChatContainer` regardless of display mode or enable toggle.
     Keep the file (rename-allowed later) so the regression is locked in.
7. `src/web-ui/src/locales/zh-CN/settings/session-config.json`
   - Update `displayInputDesc` to "在会话输入框左上角显示。"
8. `src/web-ui/src/locales/zh-TW/settings/session-config.json`
   - Update `displayInputDesc` to "在會話輸入框左上角顯示。"
9. `src/web-ui/src/locales/en-US/settings/session-config.json`
   - Update `displayInputDesc` to "Show at the top-left of the chat input
     box."

## Risks

- **Pointer-event blocking**: without `pointer-events: none` on the wrapper,
  clicks at the top-left of the input box would land on the pet. The wrapper
  rule prevents this; verify via test that pointer events propagate to the
  underlying editor.
- **Subscription leak**: the `addChangeListener` returned unsubscribe must run
  in cleanup. `ChatInput` is mounted for the lifetime of the chat session, so
  leaking would only manifest in tests that mount/unmount repeatedly.
- **Capsule overflow clipping**: the pet overflows the capsule by 2 px on each
  side. The parent containers (`ContextDropZone`, the chat shell) must not
  set `overflow: hidden` on the input area. Audit existing parent SCSS during
  implementation; if a parent does clip, raise the pet's z-index above the
  clip or relax the parent's overflow.
- **i18n audit**: changing the resource string requires running
  `pnpm run i18n:audit` to keep key/placeholder parity. zh-CN / zh-TW / en-US
  must stay in sync.
- **ChatInput size**: `ChatInput.tsx` is already 3,505 lines. Adding
  ~30-40 lines of state, effects, and JSX is small relative to the file size
  and follows the existing pattern of multiple hooks at the top of the
  component.

## Verification

- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run lint`
- `pnpm run i18n:audit`
- Focused tests:
  - `pnpm --dir src/web-ui run test:run src/flow_chat/components/ChatInput.pet.test.tsx`
  - `pnpm --dir src/web-ui run test:run src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx`
  - `pnpm --dir src/web-ui run test:run src/flow_chat/components/modern` (no
    regression in the modern container suite)

Manual:

- Set `agent_companion_display_mode` to "输入框内" and confirm the pet
  appears at the top-left of the input box, 48×48, slightly overhanging
  the rounded corner in both capsule and multi-line modes.
- Start an AI turn: pet should switch from `rest` to `working` / `waiting`,
  then back to `rest` when the turn completes.
- Switch to "悬浮桌面宠物": the input-box pet disappears, the floating
  window pet appears (existing behavior, unchanged).
- Click on the input area where the pet sits: text cursor still enters the
  editor (the pet wrapper is `pointer-events: none`).
- Confirm the pet does not block message-list clicks (no longer rendered
  in the messages area).
