# Agent Companion Pet in Main Chat — Implementation Plan

> **For AI agents:** Required sub-skill: use `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task by task. Steps use checkbox
> (`- [ ]`) syntax for progress tracking.

**Goal:** Render the Agent Companion Pet at the bottom-left of the main chat message
canvas when `agent_companion_display_mode === 'input'`, so the pet is visible in the
regular chat view, not only in the floating ToolbarMode window.

**Architecture:** In `ModernFlowChatContainer`, subscribe to `aiExperienceConfigService`
with a change listener (fixes the stale-settings issue from the sibling ToolbarMode
bug), derive the pet mood directly from the active session's last turn and round
(same logic as `ToolbarMode.tsx:133-181`), and render a `<ChatInputPixelPet>` block
as a sibling of `VirtualMessageList` / `WelcomePanel` / `HistorySessionPlaceholder`
inside the `.modern-flowchat-container__messages` div. Style with absolute
positioning relative to that messages div.

**Tech stack:** React 18, TypeScript, Vitest, i18next JSON, SCSS modules, Zustand
store for session state.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx` | modify | Add settings subscription, mood derivation, pet render block |
| `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss` | modify | Add absolute-position rules for the pet wrapper |
| `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx` | create | Verify pet shows/hides on settings change, on processing state, on enable toggle |
| `src/web-ui/src/locales/zh-CN/settings/session-config.json` | modify | Update `displayInputDesc` to "在主聊天界面左下角显示" |
| `src/web-ui/src/locales/zh-TW/settings/session-config.json` | modify | Update the parallel string for Traditional Chinese |
| `src/web-ui/src/locales/en-US/settings/session-config.json` | modify | Update the parallel English string |

---

## Task 1: Update locale descriptions (resource-only)

**Files:**
- Modify: `src/web-ui/src/locales/zh-CN/settings/session-config.json:23`
- Modify: `src/web-ui/src/locales/zh-TW/settings/session-config.json:23`
- Modify: `src/web-ui/src/locales/en-US/settings/session-config.json:23`

- [ ] **Step 1.1: Update zh-CN `displayInputDesc`**

Replace the value at line 23:

```json
"displayInputDesc": "在主聊天界面左下角显示。",
```

- [ ] **Step 1.2: Update zh-TW `displayInputDesc`**

Replace the value at line 23:

```json
"displayInputDesc": "在主聊天介面左下角顯示。",
```

- [ ] **Step 1.3: Update en-US `displayInputDesc`**

Replace the value at line 23:

```json
"displayInputDesc": "Show at the bottom-left of the main chat canvas.",
```

- [ ] **Step 1.4: Run i18n audit**

Run: `pnpm run i18n:audit`
Expected: PASS (key still present, all locale resources aligned).

- [ ] **Step 1.5: Commit**

```bash
git add src/web-ui/src/locales/zh-CN/settings/session-config.json \
        src/web-ui/src/locales/zh-TW/settings/session-config.json \
        src/web-ui/src/locales/en-US/settings/session-config.json
git commit -m "i18n(settings): describe input-mode pet at main chat bottom-left"
```

---

## Task 2: Add the SCSS positioning rules

**Files:**
- Modify: `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss`

- [ ] **Step 2.1: Append a new rule block**

Append after the existing `&__history-overlay` block (line 45), before the blank
line at line 47:

```scss
  &__companion-pet {
    position: absolute;
    left: 16px;
    bottom: 16px;
    z-index: 50;
    pointer-events: none;
    cursor: default;
  }

  &__companion-pet-element {
    width: 48px;
    height: 48px;
    pointer-events: auto;
  }
```

Notes:
- `pointer-events: none` on the wrapper so clicks pass through to the message
  list beneath; the pet element itself re-enables `pointer-events: auto` to keep
  the existing pet behavior (no clicks, but no accidental block).
- `z-index: 50` is higher than the welcome panel, history overlay (10), and
  virtual list scrollbars (no explicit z, but our absolute layer wins over
  the static message list).
- `left: 16px; bottom: 16px;` matches the design's "bottom-left with 16px
  margin" requirement.

- [ ] **Step 2.2: Commit**

```bash
git add src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss
git commit -m "style(flowchat): reserve canvas for companion pet at bottom-left"
```

---

## Task 3: Write a failing test for the pet render

**Files:**
- Create: `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx`

The test will mount the container with `enable_agent_companion=true`,
`display_mode='input'`, and a non-null pet, and assert the pet element appears.

- [ ] **Step 3.1: Write the test file**

Create `ModernFlowChatContainer.pet.test.tsx` with the following content. The
file mirrors the structure of `ModernFlowChatContainer.history-state.test.tsx`:

```tsx
// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { ModernFlowChatContainer } from './ModernFlowChatContainer';
import { aiExperienceConfigService } from '@/infrastructure/config/services/AIExperienceConfigService';
import type { AIExperienceSettings } from '@/infrastructure/config/services/AIExperienceConfigService';
import type { Session } from '../../types/flow-chat';
import { flowChatStore } from '../../store/FlowChatStore';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const stateMocks = vi.hoisted(() => ({
  activeSession: null as Session | null,
  virtualItems: [] as unknown[],
  visibleTurnInfo: null as unknown,
}));

const virtualListMock = vi.hoisted(() => ({
  scrollToTurn: vi.fn(),
  scrollToIndex: vi.fn(),
  scrollToPhysicalBottomAndClearPin: vi.fn(),
  scrollToTurnEndAndClearPin: vi.fn(() => true),
  scrollToLatestEndPosition: vi.fn(),
  isTurnRenderedInViewport: vi.fn(() => false),
  isTurnTextRenderedInViewport: vi.fn(() => false),
  pinTurnToTop: vi.fn(() => true),
}));

const startupTraceMock = vi.hoisted(() => ({
  markPhase: vi.fn(),
}));

const searchStateMock = vi.hoisted(() => ({
  searchQuery: '',
  onSearchChange: vi.fn(),
  matches: [] as unknown[],
  matchIndices: [] as number[],
  currentMatchIndex: -1,
  currentMatchVirtualIndex: -1,
  goToNext: vi.fn(),
  goToPrev: vi.fn(),
  clearSearch: vi.fn(),
}));

const headerPropsMock = vi.hoisted(() => ({
  latest: null as Record<string, unknown> | null,
}));

const chatInputPetRenderMock = vi.hoisted(() => ({
  lastMood: null as string | null,
  lastPet: null as unknown,
}));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: () => undefined },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/infrastructure/hooks/useShortcut', () => ({ useShortcut: vi.fn() }));
vi.mock('@/flow_chat/services/FlowChatManager', () => ({
  FlowChatManager: {
    getInstance: () => ({
      cancelCurrentTask: vi.fn(),
      createChatSession: vi.fn(),
      switchChatSession: vi.fn(),
    }),
  },
}));
vi.mock('@/app/stores/sessionModeStore', () => ({
  useSessionModeStore: { getState: () => ({ setMode: vi.fn() }) },
}));
vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({ workspacePath: 'D:/workspace/BitFun' }),
}));
vi.mock('../../utils/acpSession', () => ({ isAcpFlowSession: () => false }));
vi.mock('../../store/modernFlowChatStore', () => ({
  useVirtualItems: () => stateMocks.virtualItems,
  useActiveSession: () => stateMocks.activeSession,
  useVisibleTurnInfo: () => stateMocks.visibleTurnInfo,
}));
vi.mock('./VirtualMessageList', () => ({
  VirtualMessageList: React.forwardRef((_, ref) => {
    React.useImperativeHandle(ref, () => virtualListMock);
    return <div data-testid="virtual-list" />;
  }),
}));
vi.mock('@/shared/utils/startupTrace', () => ({ startupTrace: startupTraceMock }));
vi.mock('./FlowChatHeader', () => ({
  FlowChatHeader: (props: Record<string, unknown>) => {
    headerPropsMock.latest = props;
    return <div data-testid="flowchat-header" />;
  },
}));
vi.mock('../WelcomePanel', () => ({
  WelcomePanel: () => <div data-testid="welcome-panel">Welcome</div>,
}));
vi.mock('./useExploreGroupState', () => ({
  useExploreGroupState: () => ({
    exploreGroupStates: {},
    onExploreGroupToggle: vi.fn(),
    onExpandGroup: vi.fn(),
    onExpandAllInTurn: vi.fn(),
    onCollapseGroup: vi.fn(),
  }),
}));
vi.mock('./useFlowChatFileActions', () => ({
  useFlowChatFileActions: () => ({ handleFileViewRequest: vi.fn() }),
}));
vi.mock('./useFlowChatNavigation', () => ({ useFlowChatNavigation: vi.fn() }));
vi.mock('./useFlowChatCopyDialog', () => ({ useFlowChatCopyDialog: vi.fn() }));
vi.mock('./useFlowChatSync', () => ({ useFlowChatSync: vi.fn() }));
vi.mock('./useFlowChatToolActions', () => ({
  useFlowChatToolActions: () => ({
    handleToolConfirm: vi.fn(),
    handleToolReject: vi.fn(),
  }),
}));
vi.mock('./useFlowChatSearch', () => ({
  useFlowChatSearch: () => searchStateMock,
}));

vi.mock('../ChatInputPixelPet', () => ({
  ChatInputPixelPet: (props: { mood: string; pet: unknown; className?: string }) => {
    chatInputPetRenderMock.lastMood = props.mood;
    chatInputPetRenderMock.lastPet = props.pet;
    return (
      <div
        data-testid="chat-input-pet"
        data-mood={props.mood}
        className={props.className}
      />
    );
  },
}));

const pet = {
  id: 'preset:panda',
  displayName: 'Panda',
  source: 'preset' as const,
  packagePath: '/pet',
  spritesheetPath: '/sprite.png',
  spritesheetMimeType: 'image/png',
};

let savedSettings: AIExperienceSettings | null = null;

function setServiceSettings(overrides: Partial<AIExperienceSettings>): void {
  const current = aiExperienceConfigService.getSettings();
  const next: AIExperienceSettings = { ...current, ...overrides };
  savedSettings = next;
  vi.spyOn(aiExperienceConfigService, 'getSettings').mockReturnValue(next);
  vi.spyOn(aiExperienceConfigService, 'getSettingsAsync').mockResolvedValue(next);
}

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    title: 'Test session',
    dialogTurns: [],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    isHistorical: false,
    todos: [],
    mode: 'agentic',
    workspacePath: 'D:/workspace/BitFun',
    sessionKind: 'normal',
    ...overrides,
  };
}

describe('ModernFlowChatContainer agent companion pet', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    chatInputPetRenderMock.lastMood = null;
    chatInputPetRenderMock.lastPet = null;
    stateMocks.activeSession = null;
    stateMocks.virtualItems = [];
    stateMocks.visibleTurnInfo = null;
    setServiceSettings({
      enable_agent_companion: true,
      agent_companion_display_mode: 'input',
      agent_companion_pet: pet,
    });
  });

  afterEach(() => {
    if (root) {
      act(() => { root.unmount(); });
    }
    container?.remove();
    vi.restoreAllMocks();
  });

  it('renders the pet at the bottom-left of the canvas when input mode is active', () => {
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    const petElement = container.querySelector('[data-testid="chat-input-pet"]');
    expect(petElement).not.toBeNull();
    const wrapper = petElement?.parentElement;
    expect(wrapper?.className).toContain('modern-flowchat-container__companion-pet');
  });

  it('does not render the pet when display mode is desktop', () => {
    setServiceSettings({ agent_companion_display_mode: 'desktop' });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    expect(container.querySelector('[data-testid="chat-input-pet"]')).toBeNull();
  });

  it('does not render the pet when the agent companion toggle is off', () => {
    setServiceSettings({ enable_agent_companion: false });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    expect(container.querySelector('[data-testid="chat-input-pet"]')).toBeNull();
  });
});
```

- [ ] **Step 3.2: Run the new test and confirm it fails**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx
```
Expected: FAIL — `data-testid="chat-input-pet"` not found, because the production
code does not render the pet yet.

---

## Task 4: Implement the pet render in `ModernFlowChatContainer`

**Files:**
- Modify: `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx`

- [ ] **Step 4.1: Add imports**

After the existing imports, add (place near the other `@/infrastructure` imports):

```ts
import { aiExperienceConfigService, type AIExperienceSettings } from '@/infrastructure/config/services/AIExperienceConfigService';
import { ChatInputPixelPet } from '../ChatInputPixelPet';
```

- [ ] **Step 4.2: Add the settings state and subscription**

In the component body, after the existing `useState`/`useRef` block (around line 196),
add:

```ts
  const [companionSettings, setCompanionSettings] = useState<AIExperienceSettings>(() =>
    aiExperienceConfigService.getSettings()
  );

  useEffect(() => {
    void aiExperienceConfigService.getSettingsAsync().then(setCompanionSettings);
    const unsubscribe = aiExperienceConfigService.addChangeListener((settings) => {
      setCompanionSettings(settings);
    });
    return () => {
      unsubscribe();
    };
  }, []);
```

- [ ] **Step 4.3: Derive `showCompanionPet` and `companionMood`**

Add immediately after the new useEffect:

```ts
  const showCompanionPet = useMemo(
    () =>
      companionSettings.enable_agent_companion &&
      companionSettings.agent_companion_display_mode === 'input' &&
      Boolean(companionSettings.agent_companion_pet),
    [companionSettings]
  );

  const companionMood = useMemo(() => {
    if (!showCompanionPet) return 'rest' as const;
    if (!activeSession || activeSession.dialogTurns.length === 0) {
      return 'rest' as const;
    }
    const lastTurn = activeSession.dialogTurns[activeSession.dialogTurns.length - 1];
    const isProcessing =
      lastTurn.status === 'processing' ||
      lastTurn.status === 'finishing' ||
      lastTurn.status === 'image_analyzing';
    if (!isProcessing) return 'rest' as const;
    if (!lastTurn.modelRounds || lastTurn.modelRounds.length === 0) {
      return 'working' as const;
    }
    const lastRound = lastTurn.modelRounds[lastTurn.modelRounds.length - 1];
    for (let i = lastRound.items.length - 1; i >= 0; i -= 1) {
      const item = lastRound.items[i];
      if (item.type === 'tool' && 'toolName' in item && (item as { toolName?: string }).toolName) {
        return 'waiting' as const;
      }
    }
    return 'working' as const;
  }, [activeSession, showCompanionPet]);
```

- [ ] **Step 4.4: Render the pet block**

In the JSX, locate the messages container (line 856 onward) and append the pet
node as the last child of `.modern-flowchat-container__messages`. Specifically,
after line 898 (the closing fragment of the conditional render block) and before
line 899 (the closing `</div>` of the messages container), add:

```tsx
          {showCompanionPet && (
            <div
              className="modern-flowchat-container__companion-pet"
              data-testid="modern-flowchat-companion-pet"
            >
              <ChatInputPixelPet
                mood={companionMood}
                pet={companionSettings.agent_companion_pet}
                className="modern-flowchat-container__companion-pet-element"
              />
            </div>
          )}
```

Important: ensure this node sits **inside** the messages div (`<div className="modern-flowchat-container__messages" ...>` at line 857) and is a sibling of the `VirtualMessageList` / `WelcomePanel` / `HistorySessionPlaceholder` block. Keep the indentation consistent with the surrounding JSX.

- [ ] **Step 4.5: Run the new test and confirm it passes**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx
```
Expected: PASS (3 tests, all green).

- [ ] **Step 4.6: Run the full modern-container test suite to ensure no regression**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/modern
```
Expected: All previously-green tests stay green; the new tests stay green.

- [ ] **Step 4.7: Type-check**

Run: `pnpm run type-check:web`
Expected: PASS.

- [ ] **Step 4.8: Commit**

```bash
git add src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx \
        src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx
git commit -m "feat(flowchat): render agent companion pet in main chat canvas"
```

---

## Task 5: Manual smoke (recorded, not automated)

The dev environment is not reachable from the agent. Record what the human should
verify in the PR description.

- [ ] **Step 5.1: List manual checks in the PR description**

PR description should list:

- Switch `agent_companion_display_mode` to "输入框内" and confirm the pet
  appears at the bottom-left of the main chat canvas, 48×48, with 16px margin.
- Start an AI turn: pet should switch from `rest` to `working` / `waiting`,
  then back to `rest` when the turn completes.
- Switch to "悬浮桌面宠物": the canvas pet disappears, the floating window
  pet appears (existing behavior, unchanged).
- Switch to the ToolbarMode (悬浮窗模式): the canvas pet is not visible in
  the ToolbarMode window (the ToolbarMode keeps its own pet render path).
- Confirm the pet does not block message-list clicks (the wrapper is
  `pointer-events: none`).

---

## Self-Check

1. **Spec coverage:**
   - Render point (ModernFlowChatContainer `__messages` div) — Task 4.4
   - Visibility predicate — Task 4.3 (`showCompanionPet` mirrors ToolbarMode:78-83)
   - Mood from streaming/tool state — Task 4.3 (mirrors ToolbarMode:72-76 logic)
   - Settings subscription with listener (fixes ToolbarMode stale-settings bug) — Task 4.2
   - Layout 48×48 bottom-left, z-index 50 — Task 2.1 + Task 4.4
   - Locale updates zh-CN / zh-TW / en-US — Task 1.1, 1.2, 1.3
   - Tests covering show/hide paths — Task 3.1

2. **Placeholder scan:** No "TODO" / "TBD" / "similar to task X" markers.

3. **Type consistency:**
   - `AIExperienceSettings` is imported from the same module that exports
     `aiExperienceConfigService` (Task 4.1).
   - `ChatInputPixelPet` `mood` prop accepts `'rest' | 'analyzing' | 'waiting' | 'working'`
     per `ChatInputPetMood`. The plan only emits `rest` / `working` / `waiting`,
     which are all valid. Use `as const` so TypeScript narrows the literals.
   - `companionSettings.agent_companion_pet` is typed as
     `AgentCompanionPetSelection | null | undefined`. `ChatInputPixelPet`'s `pet`
     prop accepts `AgentCompanionPetSelection | null`, so passing it through is
     safe (undefined becomes null in the falsy check inside `showCompanionPet`).

No gaps; proceed to execution.
