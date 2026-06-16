# Agent Companion Pet at ChatInput Top-Left — Implementation Plan

> **For AI agents:** Required sub-skill: use `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task by task. Steps use checkbox
> (`- [ ]`) syntax for progress tracking.

**Goal:** Move the Agent Companion Pet's "input" display mode render point
from the bottom-left of the main chat canvas in `ModernFlowChatContainer` to
the top-left corner of the chat input box in `ChatInput`, so the pet visually
hangs at the top-left of the input field and matches the i18n description.

**Architecture:** Subscribe to `aiExperienceConfigService` inside
`ChatInput` (with a change listener that fixes the stale-settings issue
already noted in the project's `KNOWN_ISSUES` for the sibling
`ToolbarMode`), derive `companionMood` from the active session's last turn
(identical logic to the previous `ModernFlowChatContainer` derivation), and
render a `<ChatInputPixelPet>` as the first child of `bitfun-chat-input__box`
so the pet anchors to the input box's top-left. Style with absolute
positioning relative to `__box`; allow 48×48 to overflow the capsule
(44 px) and the rounded corner. Remove the previous `ModernFlowChatContainer`
pet render block, SCSS, and imports, and rewrite the regression test to
assert the pet is no longer rendered in the messages area.

**Tech stack:** React 18, TypeScript, Vitest, i18next JSON, SCSS, Zustand
store for session state.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `src/web-ui/src/flow_chat/components/ChatInput.tsx` | modify | Add settings subscription, mood derivation, pet render block as first child of `__box` |
| `src/web-ui/src/flow_chat/components/ChatInput.scss` | modify | Add `&__companion-pet` absolute-position rules |
| `src/web-ui/src/flow_chat/components/ChatInput.pet.test.tsx` | create | Verify pet shows/hides on settings change, on processing state, on enable toggle, and lives inside `__box` |
| `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx` | modify | Remove the subscription, derivations, render block, and unused imports |
| `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss` | modify | Remove the `&__companion-pet` and `&__companion-pet-element` rules |
| `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx` | modify | Rewrite to assert the pet is NOT rendered inside `ModernFlowChatContainer` regardless of display mode |
| `src/web-ui/src/locales/zh-CN/settings/session-config.json` | modify | Update `displayInputDesc` to "在会话输入框左上角显示。" |
| `src/web-ui/src/locales/zh-TW/settings/session-config.json` | modify | Update `displayInputDesc` to "在會話輸入框左上角顯示。" |
| `src/web-ui/src/locales/en-US/settings/session-config.json` | modify | Update `displayInputDesc` to "Show at the top-left of the chat input box." |

---

## Task 1: Update locale descriptions (resource-only)

**Files:**
- Modify: `src/web-ui/src/locales/zh-CN/settings/session-config.json:23`
- Modify: `src/web-ui/src/locales/zh-TW/settings/session-config.json:23`
- Modify: `src/web-ui/src/locales/en-US/settings/session-config.json:23`

- [ ] **Step 1.1: Update zh-CN `displayInputDesc`**

Replace the value at line 23 (the string under `agentCompanion.displayInputDesc`):

```json
"displayInputDesc": "在会话输入框左上角显示。",
```

- [ ] **Step 1.2: Update zh-TW `displayInputDesc`**

Replace the value at line 23:

```json
"displayInputDesc": "在會話輸入框左上角顯示。",
```

- [ ] **Step 1.3: Update en-US `displayInputDesc`**

Replace the value at line 23:

```json
"displayInputDesc": "Show at the top-left of the chat input box.",
```

- [ ] **Step 1.4: Run i18n audit**

Run: `pnpm run i18n:audit`
Expected: PASS (key still present, all locale resources aligned).

- [ ] **Step 1.5: Commit**

```bash
git add src/web-ui/src/locales/zh-CN/settings/session-config.json \
        src/web-ui/src/locales/zh-TW/settings/session-config.json \
        src/web-ui/src/locales/en-US/settings/session-config.json
git commit -m "i18n(settings): describe input-mode pet at chat input top-left"
```

---

## Task 2: Add the SCSS positioning rules in ChatInput

**Files:**
- Modify: `src/web-ui/src/flow_chat/components/ChatInput.scss`

- [ ] **Step 2.1: Append the companion-pet rule block**

Locate the `&__box` block (starts at line 322 in the current file) and append
the new rule immediately after the closing brace of `&__box` (after line 430
in the current file). The new block:

```scss
  &__companion-pet {
    position: absolute;
    top: 0;
    left: 0;
    z-index: 10;
    width: 48px;
    height: 48px;
    pointer-events: none;
    cursor: default;

    > * {
      pointer-events: auto;
    }
  }
```

Notes:

- `pointer-events: none` on the wrapper so clicks pass through to the text
  editor underneath; the pet's own element re-enables `pointer-events: auto`
  to preserve the existing pet hover/animation behavior.
- `z-index: 10` is higher than the placeholder text (no explicit z) and
  the agent-boost capsule in `__actions` (no explicit z). The pet
  visually anchors to the top-left corner.
- The 48×48 size is fixed in both capsule (44 px) and multi-line (≥ 76 px)
  modes. Overflow is allowed because `__box` has no explicit `overflow`
  rule (default `visible`).
- The pet wrapper is the first child of `__box` so it sits at the
  top-left and is not pushed by any sibling sections.

- [ ] **Step 2.2: Commit**

```bash
git add src/web-ui/src/flow_chat/components/ChatInput.scss
git commit -m "style(chat-input): reserve top-left for companion pet"
```

---

## Task 3: Write a failing test for the ChatInput pet render

**Files:**
- Create: `src/web-ui/src/flow_chat/components/ChatInput.pet.test.tsx`

`ChatInput.tsx` is a 3,505-line component with many dependencies
(`RichTextInput`, `ContextDropZone`, `FileMentionPicker`, `AcpPlanPanel`,
`ModelSelector`, etc.). Mounting the full component in a unit test would
require mocking the entire dependency graph. The cleanest approach is to
mount the production `ChatInput` with the same heavy mocks used by the
sibling `ModernFlowChatContainer.pet.test.tsx`, and assert the
`data-testid="chat-input-pet"` element appears inside the
`bitfun-chat-input__box` div.

- [ ] **Step 3.1: Write the test file**

Create `ChatInput.pet.test.tsx` with the following content:

```tsx
// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { ChatInput } from './ChatInput';
import {
  aiExperienceConfigService,
  type AIExperienceSettings,
} from '@/infrastructure/config/services/AIExperienceConfigService';
import type { Session } from '../types/flow-chat';
import { flowChatStore } from '../store/FlowChatStore';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const stateMocks = vi.hoisted(() => ({
  activeSession: null as Session | null,
  showRecommendations: false,
  recommendationContext: null as unknown,
  isMultiLine: false,
  isProcessing: false,
  inputTarget: 'main' as 'main' | 'btw',
  showTargetSwitcher: false,
  effectiveTargetSessionId: 'session-1' as string | null,
  currentSessionTitle: 'Test session',
  activeBtwTargetLabel: '',
  activeBtwSessionTitle: '',
  showPlaceholder: true,
  showInputArea: true,
  imageContexts: [] as unknown[],
}));

const chatInputPetRenderMock = vi.hoisted(() => ({
  lastMood: null as string | null,
  lastPet: null as unknown,
}));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: () => undefined },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/flow_chat/hooks', () => ({
  useActiveSessionState: () => ({
    activeSession: stateMocks.activeSession,
    derivedState: {
      isProcessing: stateMocks.isProcessing,
    },
  }),
}));

vi.mock('@/shared/context-system', () => ({
  ContextDropZone: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="context-drop-zone">{children}</div>
  ),
  useContextStore: () => ({
    imageContexts: stateMocks.imageContexts,
    removeContext: vi.fn(),
  }),
}));

vi.mock('./RichTextInput', () => ({
  RichTextInput: React.forwardRef((_, ref) => {
    React.useImperativeHandle(ref, () => ({}));
    return <div data-testid="rich-text-input" />;
  }),
}));

vi.mock('./FileMentionPicker', () => ({
  FileMentionPicker: () => null,
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: { on: vi.fn(), off: vi.fn() },
}));

vi.mock('../hooks/useSessionStateMachine', () => ({
  useSessionDerivedState: () => ({}),
  useSessionStateMachine: () => ({}),
  useSessionStateMachineActions: () => ({}),
}));

vi.mock('../state-machine/types', () => ({
  SessionExecutionEvent: {},
}));

vi.mock('./ModelSelector', () => ({
  ModelSelector: () => <div data-testid="model-selector" />,
}));

vi.mock('../store/FlowChatStore', () => ({
  FlowChatStore: { getState: () => ({}) },
}));

vi.mock('../hooks/useAcpPlan', () => ({
  useAcpPlan: () => ({ entries: [] }),
}));

vi.mock('../hooks/useAcpSlashCommands', () => ({
  filterSlashCommands: vi.fn(() => []),
  useAcpSlashCommands: () => ({ commands: [] }),
}));

vi.mock('../utils/acpSession', () => ({
  acpSessionRef: { current: null },
  acpSlashCommandText: '',
}));

vi.mock('./AcpPlanPanel', () => ({
  AcpPlanPanel: () => null,
}));

vi.mock('./smart-recommendations', () => ({
  SmartRecommendations: () => null,
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useCurrentWorkspace: () => ({ kind: 'local' }),
}));

vi.mock('@/shared/types', () => ({
  WorkspaceKind: { Local: 'local' },
}));

vi.mock('../utils/imageUtils', () => ({
  createImageContextFromFile: vi.fn(),
  createImageContextFromClipboard: vi.fn(),
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: { warning: vi.fn() },
}));

vi.mock('../reducers/inputReducer', () => ({
  inputReducer: (state: unknown) => state,
  initialInputState: {},
}));

vi.mock('../reducers/modeReducer', () => ({
  modeReducer: (state: unknown) => state,
  initialModeState: { current: 'agentic' },
}));

vi.mock('../constants/chatInputConfig', () => ({
  CHAT_INPUT_CONFIG: { image: { maxCount: 4 } },
}));

vi.mock('../hooks/useMessageSender', () => ({
  useMessageSender: () => ({ send: vi.fn() }),
}));

vi.mock('../store/chatInputStateStore', () => ({
  useChatInputState: () => ({}),
}));

vi.mock('../store/inputHistoryStore', () => ({
  useInputHistoryStore: () => ({}),
}));

vi.mock('../services/BtwThreadService', () => ({
  startBtwThread: vi.fn(),
}));

vi.mock('../services/usageReportService', () => ({
  runUsageReportCommand: vi.fn(),
}));

vi.mock('../services/goalService', () => ({
  isGoalSlashCommand: () => false,
  parseGoalCommand: () => ({}),
}));

vi.mock('../hooks/useThreadGoalController', () => ({
  useThreadGoalController: () => ({}),
}));

vi.mock('./thread-goal/ThreadGoalDialogs', () => ({
  ThreadGoalDialogs: () => null,
}));

vi.mock('@/flow_chat', () => ({
  FlowChatManager: { getInstance: () => ({}) },
}));

vi.mock('../services/DeepReviewService', () => ({
  DEEP_REVIEW_SLASH_COMMAND: '',
  getDeepReviewLaunchErrorMessage: () => '',
  buildDeepReviewLaunchFromSlashCommand: () => ({}),
  buildDeepReviewPreviewFromSlashCommand: () => ({}),
  isDeepReviewSlashCommand: () => false,
  launchDeepReviewSession: vi.fn(),
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  IconButton: ({ children, ...rest }: { children: React.ReactNode }) => (
    <button {...rest}>{children}</button>
  ),
  confirmWarning: vi.fn(),
}));

vi.mock('./PendingQueuePanel', () => ({
  PendingQueuePanel: () => null,
}));

vi.mock('@/app/components/panels/content-canvas/stores', () => ({
  useAgentCanvasStore: { getState: () => ({}) },
}));

vi.mock('../services/openBtwSession', () => ({
  openBtwSessionInAuxPane: vi.fn(),
  selectActiveBtwSessionTab: vi.fn(),
}));

vi.mock('../utils/sessionMetadata', () => ({
  resolveSessionRelationship: () => null,
}));

vi.mock('../utils/chatInputMode', () => ({
  resolveWorkspaceChatInputMode: () => 'agentic',
}));

vi.mock('@/app/stores/sceneStore', () => ({
  useSceneStore: { getState: () => ({}) },
}));

vi.mock('@/app/components/SceneBar/types', () => ({
}));

vi.mock('@/infrastructure/api', () => ({
  configAPI: {},
}));

vi.mock('@/infrastructure/config/types', () => ({}));

vi.mock('@/infrastructure/api/service-api/MCPAPI', () => ({
  default: {},
}));

vi.mock('./ChatInputWorkspaceStrip', () => ({
  ChatInputWorkspaceStrip: () => null,
}));

vi.mock('@/tools/generative-widget/widgetPromptReference', () => ({
  expandWidgetPromptReferenceTokens: (s: string) => s,
}));

vi.mock('./DeepReviewConsentDialog', () => ({
  useDeepReviewConsent: () => ({}),
}));

vi.mock('../hooks/useSessionReviewActivity', () => ({
  useSessionReviewActivity: () => ({}),
}));

vi.mock('../utils/deepReviewCommandGuard', () => ({
  shouldBlockDeepReviewCommand: () => false,
}));

vi.mock('../utils/deepReviewCapacityGuard', () => ({
  deriveDeepReviewSessionConcurrencyGuard: () => ({}),
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {},
}));

vi.mock('../hooks/useVoiceInput', () => ({
  useVoiceInputToChat: () => ({
    isListening: false,
    toggle: vi.fn(),
  }),
}));

vi.mock('./chatPopupState', () => ({
  setChatPopupActive: vi.fn(),
}));

vi.mock('./ChatInputPixelPet', () => ({
  ChatInputPixelPet: (props: { mood: string; pet: unknown }) => {
    chatInputPetRenderMock.lastMood = props.mood;
    chatInputPetRenderMock.lastPet = props.pet;
    return <div data-testid="chat-input-pet" data-mood={props.mood} />;
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

function setServiceSettings(overrides: Partial<AIExperienceSettings>): void {
  const current = aiExperienceConfigService.getSettings();
  const next: AIExperienceSettings = { ...current, ...overrides };
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

describe('ChatInput agent companion pet', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    chatInputPetRenderMock.lastMood = null;
    chatInputPetRenderMock.lastPet = null;
    stateMocks.activeSession = null;
    stateMocks.isProcessing = false;
    stateMocks.isMultiLine = false;
    stateMocks.showTargetSwitcher = false;
    stateMocks.showRecommendations = false;
    stateMocks.imageContexts = [];
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

  it('renders the pet at the top-left of the input box when input mode is active', () => {
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ChatInput />); });

    const petElement = container.querySelector('[data-testid="chat-input-pet"]');
    expect(petElement).not.toBeNull();
    const wrapper = petElement?.parentElement;
    expect(wrapper?.className).toContain('bitfun-chat-input__companion-pet');
    // The wrapper should live inside bitfun-chat-input__box so it anchors
    // to the input box's top-left.
    let node: HTMLElement | null = wrapper;
    let insideBox = false;
    while (node) {
      if (node.className?.includes?.('bitfun-chat-input__box')) {
        insideBox = true;
        break;
      }
      node = node.parentElement;
    }
    expect(insideBox).toBe(true);
  });

  it('does not render the pet when display mode is desktop', () => {
    setServiceSettings({ agent_companion_display_mode: 'desktop' });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ChatInput />); });

    expect(container.querySelector('[data-testid="chat-input-pet"]')).toBeNull();
  });

  it('does not render the pet when the agent companion toggle is off', () => {
    setServiceSettings({ enable_agent_companion: false });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ChatInput />); });

    expect(container.querySelector('[data-testid="chat-input-pet"]')).toBeNull();
  });

  it('emits working mood when the active turn is processing with no tool', () => {
    stateMocks.activeSession = createSession({
      dialogTurns: [
        {
          id: 'turn-1',
          userMessage: { content: 'hi' },
          status: 'processing',
          modelRounds: [],
        } as never,
      ],
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ChatInput />); });

    expect(chatInputPetRenderMock.lastMood).toBe('working');
  });

  it('emits waiting mood when the latest round has a tool', () => {
    stateMocks.activeSession = createSession({
      dialogTurns: [
        {
          id: 'turn-1',
          userMessage: { content: 'hi' },
          status: 'processing',
          modelRounds: [
            {
              id: 'round-1',
              items: [
                { type: 'tool', toolName: 'search', id: 'tool-1' } as never,
              ],
            },
          ],
        } as never,
      ],
    });
    container = document.testid = '';
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ChatInput />); });

    expect(chatInputPetRenderMock.lastMood).toBe('waiting');
  });
});
```

Note on the mock list: the implementer should adjust the mock list if a new
dependency surfaces during the test run. The principle is to mock everything
the production component imports at the top of the file. Run the test (Step
3.2) to discover any unmocked dependencies and add them.

- [ ] **Step 3.2: Run the new test and confirm it fails**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/ChatInput.pet.test.tsx
```

Expected: FAIL — `data-testid="chat-input-pet"` not found, because the
production code does not render the pet yet. If the test fails with
"X is not a function" or "Cannot read properties of undefined", add the
required mocks to the `vi.mock(...)` block in Step 3.1 and re-run.

- [ ] **Step 3.3: Commit the failing test (TDD red)**

```bash
git add src/web-ui/src/flow_chat/components/ChatInput.pet.test.tsx
git commit -m "test(chat-input): cover companion pet at top-left of input box"
```

---

## Task 4: Implement the pet render in `ChatInput`

**Files:**
- Modify: `src/web-ui/src/flow_chat/components/ChatInput.tsx`

- [ ] **Step 4.1: Add imports**

Locate the import block near the top of `ChatInput.tsx` (search for
`useActiveSessionState` from `@/flow_chat/hooks` — it's already imported).
Add the following imports adjacent to the existing
`@/infrastructure` imports:

```ts
import {
  aiExperienceConfigService,
  type AIExperienceSettings,
} from '@/infrastructure/config/services/AIExperienceConfigService';
import { ChatInputPixelPet } from './ChatInputPixelPet';
import type { ChatInputPetMood } from '../utils/chatInputPetMood';
```

- [ ] **Step 4.2: Add the settings state and subscription**

In the component body, after the existing `useState` / `useRef` / `useEffect`
block (the function continues for thousands of lines; place this immediately
after the first block of `useState` declarations for `recommendationContext`
or similar top-of-component state, before the first `useCallback` / memo
derivation), add:

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

  const showCompanionPet = useMemo(
    () =>
      companionSettings.enable_agent_companion &&
      companionSettings.agent_companion_display_mode === 'input' &&
      Boolean(companionSettings.agent_companion_pet),
    [companionSettings]
  );

  const companionMood = useMemo<ChatInputPetMood>(() => {
    if (!showCompanionPet) return 'rest';
    if (!activeSession || activeSession.dialogTurns.length === 0) {
      return 'rest';
    }
    const lastTurn = activeSession.dialogTurns[activeSession.dialogTurns.length - 1];
    const isProcessing =
      lastTurn.status === 'processing' ||
      lastTurn.status === 'finishing' ||
      lastTurn.status === 'image_analyzing';
    if (!isProcessing) return 'rest';
    if (!lastTurn.modelRounds || lastTurn.modelRounds.length === 0) {
      return 'working';
    }
    const lastRound = lastTurn.modelRounds[lastTurn.modelRounds.length - 1];
    for (let i = lastRound.items.length - 1; i >= 0; i -= 1) {
      const item = lastRound.items[i];
      if (item.type === 'tool' && item.toolName) {
        return 'waiting';
      }
    }
    return 'working';
  }, [activeSession, showCompanionPet]);
```

`activeSession` is the variable returned by the existing
`useActiveSessionState` hook used inside the component. The implementer
should confirm the exact variable name by searching the file for
`useActiveSessionState` and using the destructured field name
(e.g., `const { activeSession } = useActiveSessionState(...)`).

- [ ] **Step 4.3: Render the pet block as the first child of `__box`**

Locate the `bitfun-chat-input__box` div in the JSX (around line 3006 in the
current file):

```tsx
<div className={`bitfun-chat-input__box ${isMultiLine ? 'bitfun-chat-input__box--multi-line' : 'bitfun-chat-input__box--capsule'}`}>
  {showTargetSwitcher && (
    <div className="bitfun-chat-input__target-switcher" data-testid="chat-input-target-switcher">
```

Insert the pet block as the first child of the `__box` div, before the
`{showTargetSwitcher && ...}` conditional:

```tsx
          {showCompanionPet && (
            <div
              className="bitfun-chat-input__companion-pet"
              data-testid="chat-input-pet"
            >
              <ChatInputPixelPet
                mood={companionMood}
                pet={companionSettings.agent_companion_pet}
              />
            </div>
          )}
```

The wrapper has `data-testid="chat-input-pet"` for test targeting. The pet's
own element re-enables pointer-events via the `> *` rule in the SCSS.

- [ ] **Step 4.4: Run the new test and confirm it passes**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/ChatInput.pet.test.tsx
```

Expected: PASS (5 tests, all green). If a test fails, fix the
implementation or the mock setup as needed.

- [ ] **Step 4.5: Run type-check**

Run: `pnpm run type-check:web`
Expected: PASS.

- [ ] **Step 4.6: Commit**

```bash
git add src/web-ui/src/flow_chat/components/ChatInput.tsx
git commit -m "feat(chat-input): render companion pet at top-left of input box"
```

---

## Task 5: Remove the pet render from `ModernFlowChatContainer`

**Files:**
- Modify: `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx`
- Modify: `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss`
- Modify: `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx`

- [ ] **Step 5.1: Rewrite `ModernFlowChatContainer.pet.test.tsx`**

Replace the entire contents of
`src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx`
with a regression test that asserts the pet is no longer rendered in
`ModernFlowChatContainer` regardless of `display_mode` or
`enable_agent_companion`:

```tsx
// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { ModernFlowChatContainer } from './ModernFlowChatContainer';
import {
  aiExperienceConfigService,
  type AIExperienceSettings,
} from '@/infrastructure/config/services/AIExperienceConfigService';
import type { Session } from '../../types/flow-chat';

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
  FlowChatHeader: () => <div data-testid="flowchat-header" />,
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

const pet = {
  id: 'preset:panda',
  displayName: 'Panda',
  source: 'preset' as const,
  packagePath: '/pet',
  spritesheetPath: '/sprite.png',
  spritesheetMimeType: 'image/png',
};

function setServiceSettings(overrides: Partial<AIExperienceSettings>): void {
  const current = aiExperienceConfigService.getSettings();
  const next: AIExperienceSettings = { ...current, ...overrides };
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

describe('ModernFlowChatContainer agent companion pet (removed)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    stateMocks.activeSession = null;
    stateMocks.virtualItems = [];
    stateMocks.visibleTurnInfo = null;
  });

  afterEach(() => {
    if (root) {
      act(() => { root.unmount(); });
    }
    container?.remove();
    vi.restoreAllMocks();
  });

  it('does not render the pet when input mode is active', () => {
    setServiceSettings({
      enable_agent_companion: true,
      agent_companion_display_mode: 'input',
      agent_companion_pet: pet,
    });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    expect(container.querySelector('[data-testid="chat-input-pet"]')).toBeNull();
    expect(container.querySelector('.modern-flowchat-container__companion-pet')).toBeNull();
  });

  it('does not render the pet when display mode is desktop', () => {
    setServiceSettings({
      enable_agent_companion: true,
      agent_companion_display_mode: 'desktop',
      agent_companion_pet: pet,
    });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    expect(container.querySelector('.modern-flowchat-container__companion-pet')).toBeNull();
  });

  it('does not render the pet when the agent companion toggle is off', () => {
    setServiceSettings({
      enable_agent_companion: false,
      agent_companion_display_mode: 'input',
      agent_companion_pet: pet,
    });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    expect(container.querySelector('.modern-flowchat-container__companion-pet')).toBeNull();
  });
});
```

- [ ] **Step 5.2: Run the rewritten test and confirm it fails**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx
```

Expected: FAIL — the production code currently renders the pet wrapper
`.modern-flowchat-container__companion-pet`. The test must be red before the
removal work.

- [ ] **Step 5.3: Remove the imports from `ModernFlowChatContainer.tsx`**

Locate the imports at the top of
`src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx`:

```ts
import {
  aiExperienceConfigService,
  type AIExperienceSettings,
} from '@/infrastructure/config/services/AIExperienceConfigService';
import { ChatInputPixelPet } from '../ChatInputPixelPet';
```

Delete both import statements. If `aiExperienceConfigService` or
`AIExperienceSettings` is still used elsewhere in the file (it is not, given
the previous design), keep the import — but the current implementation uses
them only for the pet.

- [ ] **Step 5.4: Remove the settings state, subscription, and derivations**

Locate the block in the component body that begins with:

```ts
  const [companionSettings, setCompanionSettings] = useState<AIExperienceSettings>(() =>
    aiExperienceConfigService.getSettings()
  );
```

Delete the entire block, including:

- The `useState<AIExperienceSettings>` declaration.
- The `useEffect` that subscribes to `addChangeListener` and unsubscribes.
- The `showCompanionPet` `useMemo`.
- The `companionMood` `useMemo`.

The block ends at the `}, [activeSession, showCompanionPet]);` line. Remove
all of it. (Search for `companionSettings` and `companionMood` to find the
exact range.)

- [ ] **Step 5.5: Remove the pet render block from the JSX**

Locate the JSX:

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

Delete the entire block (including the surrounding `{showCompanionPet && ...}`
ternary).

- [ ] **Step 5.6: Remove the SCSS rules**

In
`src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss`,
locate and delete the two rule blocks:

```scss
  &__companion-pet {
    position: absolute;
    left: 16px;
    bottom: 0;
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

Do not change any other rules in the file. Re-check the surrounding
selectors to make sure no orphan selectors remain.

- [ ] **Step 5.7: Run the rewritten test and confirm it passes**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx
```

Expected: PASS (3 tests, all green).

- [ ] **Step 5.8: Run the broader modern container test suite for no regression**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/modern
```

Expected: All previously-green tests stay green; the rewritten tests stay
green.

- [ ] **Step 5.9: Run type-check**

Run: `pnpm run type-check:web`
Expected: PASS. If TypeScript reports "Cannot find name
`aiExperienceConfigService`" or similar, re-check that the unused import
in Step 5.3 was fully removed.

- [ ] **Step 5.10: Commit**

```bash
git add src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx \
        src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.scss \
        src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx
git commit -m "refactor(flowchat): drop companion pet render from main chat canvas"
```

---

## Task 6: Final verification

- [ ] **Step 6.1: Type-check**

Run: `pnpm run type-check:web`
Expected: PASS.

- [ ] **Step 6.2: Lint**

Run: `pnpm --dir src/web-ui run lint`
Expected: PASS.

- [ ] **Step 6.3: i18n audit**

Run: `pnpm run i18n:audit`
Expected: PASS (key still present, all locale resources aligned).

- [ ] **Step 6.4: Run focused pet tests**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/ChatInput.pet.test.tsx
pnpm --dir src/web-ui run test:run src/flow_chat/components/modern/ModernFlowChatContainer.pet.test.tsx
```

Expected: Both test files green.

- [ ] **Step 6.5: Run modern container tests for no regression**

Run:
```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/modern
```

Expected: All green.

---

## Self-Check

1. **Spec coverage:**
   - Render point (`__box` first child) — Task 4.3
   - Visibility predicate — Task 4.2
   - Mood from streaming/tool state — Task 4.2 (mirrors previous
     `ModernFlowChatContainer` derivation)
   - Settings subscription with listener — Task 4.2
   - Layout 48×48 top-left of `__box`, `pointer-events: none` wrapper —
     Task 2.1 + Task 4.3
   - Locale updates zh-CN / zh-TW / en-US — Tasks 1.1, 1.2, 1.3
   - Tests covering show/hide, mood, position — Tasks 3.1, 5.1
   - Old render point removed — Tasks 5.3, 5.4, 5.5, 5.6

2. **Placeholder scan:** No "TODO" / "TBD" / "similar to task X" markers.
   All step content is concrete code or commands.

3. **Type consistency:**
   - `AIExperienceSettings` is imported from the same module that exports
     `aiExperienceConfigService` (Task 4.1).
   - `ChatInputPixelPet` `mood` prop accepts the same set as
     `ChatInputPetMood`. The plan only emits `rest` / `working` / `waiting`,
     which are all valid. The `companionMood` useMemo is typed
     `useMemo<ChatInputPetMood>(...)` so TypeScript narrows the literals.
   - `companionSettings.agent_companion_pet` is typed as
     `AgentCompanionPetSelection | null | undefined`. `ChatInputPixelPet`'s
     `pet` prop accepts `AgentCompanionPetSelection | null`, so passing it
     through is safe.
   - The pet wrapper's `data-testid="chat-input-pet"` is the same id used
     in the original ToolbarMode (the test mirrors the test id contract
     for the new location).

No gaps; proceed to execution.
