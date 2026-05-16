import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { SessionFilesBadge } from './SessionFilesBadge';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  files: [] as Array<{ filePath: string; sessionId: string }>,
  getSessionFileDiffStats: vi.fn(),
  getOperationDiff: vi.fn(),
  flowState: {
    sessions: new Map<string, unknown>(),
  },
  flowListeners: new Set<() => void>(),
  settingsListeners: new Set<(settings: { quick_actions: unknown[] }) => void>(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'sessionFilesBadge.filesSummaryCount') {
        return `${String(options?.count ?? 0)} files`;
      }
      return typeof options?.defaultValue === 'string' ? options.defaultValue : key;
    },
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('../../../tools/snapshot_system/hooks/useSnapshotState', () => ({
  useSnapshotState: () => ({
    files: mocks.files,
  }),
}));

vi.mock('../../../shared/utils/tabUtils', () => ({
  createDiffEditorTab: vi.fn(),
}));

vi.mock('../../../infrastructure/api', () => ({
  snapshotAPI: {
    getSessionFileDiffStats: mocks.getSessionFileDiffStats,
    getOperationDiff: mocks.getOperationDiff,
  },
}));

vi.mock('../../../infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({
    currentWorkspace: { rootPath: 'D:/workspace/project' },
  }),
}));

vi.mock('../../../shared/notification-system', () => ({
  notificationService: {
    warning: vi.fn(),
    info: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('../../services/BtwThreadService', () => ({
  createBtwChildSession: vi.fn(),
}));

vi.mock('../../services/openBtwSession', () => ({
  openBtwSessionInAuxPane: vi.fn(),
}));

vi.mock('../../services/DeepReviewService', () => ({
  buildDeepReviewLaunchFromSessionFiles: vi.fn(),
  buildDeepReviewPreviewFromSessionFiles: vi.fn(),
  launchDeepReviewSession: vi.fn(),
}));

vi.mock('../../services/ReviewSessionMarkerService', () => ({
  insertReviewSessionSummaryMarker: vi.fn(),
}));

vi.mock('../DeepReviewConsentDialog', () => ({
  useDeepReviewConsent: () => ({
    confirmDeepReviewLaunch: vi.fn(),
    deepReviewConsentDialog: null,
  }),
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => mocks.flowState,
    subscribe: (listener: () => void) => {
      mocks.flowListeners.add(listener);
      return () => mocks.flowListeners.delete(listener);
    },
  },
}));

vi.mock('../../hooks/useSessionReviewActivity', () => ({
  useSessionReviewActivity: () => null,
}));

vi.mock('../../hooks/useSessionStateMachine', () => ({
  useSessionStateMachine: () => null,
}));

vi.mock('@/infrastructure/config/services/AIExperienceConfigService', () => ({
  DEFAULT_QUICK_ACTIONS: [],
  aiExperienceConfigService: {
    getSettings: () => ({ quick_actions: [] }),
    addChangeListener: (listener: (settings: { quick_actions: unknown[] }) => void) => {
      mocks.settingsListeners.add(listener);
      return () => mocks.settingsListeners.delete(listener);
    },
  },
}));

vi.mock('@/infrastructure/config/services/quickActionLocalization', () => ({
  resolveQuickActionText: vi.fn(),
}));

vi.mock('../../utils/deepReviewCapacityGuard', () => ({
  deriveDeepReviewSessionConcurrencyGuard: vi.fn(),
}));

describe('SessionFilesBadge', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();

    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('CustomEvent', dom.window.CustomEvent);

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);

    mocks.files = [
      { filePath: 'src/current-session.ts', sessionId: 'session-1' },
      { filePath: 'src/stale-session.ts', sessionId: 'session-1' },
    ];
    mocks.flowState.sessions = new Map<string, unknown>([
      ['session-1', {
        sessionId: 'session-1',
        dialogTurns: [],
      }],
    ]);
    mocks.flowListeners.clear();
    mocks.settingsListeners.clear();
    mocks.getSessionFileDiffStats.mockReset();
    mocks.getSessionFileDiffStats.mockResolvedValue({
      linesAdded: 1,
      linesRemoved: 0,
      changeKind: 'modify',
    });
    mocks.getOperationDiff.mockReset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('removes cached stats when the current session file list shrinks', async () => {
    await act(async () => {
      root.render(<SessionFilesBadge sessionId="session-1" />);
    });

    await act(async () => {
      vi.advanceTimersByTime(350);
      await Promise.resolve();
    });

    const toggle = container.querySelector('.session-files-badge__button') as HTMLButtonElement | null;
    expect(toggle).not.toBeNull();

    await act(async () => {
      toggle?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(container.textContent).toContain('2 files');

    mocks.files = [
      { filePath: 'src/current-session.ts', sessionId: 'session-1' },
    ];

    await act(async () => {
      root.render(<SessionFilesBadge sessionId="session-1" />);
    });

    await act(async () => {
      vi.advanceTimersByTime(350);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('1 files');
    expect(container.textContent).not.toContain('stale-session.ts');
  });
});
