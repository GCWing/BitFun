// @vitest-environment jsdom

/**
 * The two things the overlay does that the service cannot be asked about: it
 * takes Escape away from the container, and opening the panel closes it.
 */

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import { isChatPopupActive } from '../chatPopupState';
import {
  closeSessionUsageModal,
  getSessionUsageModalState,
  openSessionUsageModal,
  showSessionUsageModalReport,
} from './sessionUsageModalState';
import { SessionUsageModal } from './SessionUsageModal';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({ openSessionUsagePanel: vi.fn() }));
vi.mock('../../services/openSessionUsageReport', () => ({
  openSessionUsagePanel: mocks.openSessionUsagePanel,
}));

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');
  return { ...actual, useTranslation: () => ({ t: (key: string) => key }) };
});

const report = (): SessionUsageReport => ({
  schemaVersion: 1,
  reportId: 'usage-report-1',
  sessionId: 'session-1',
  generatedAt: 100,
  workspace: { kind: 'local', pathLabel: 'D:/workspace/OpenBitFun' },
  scope: { kind: 'entire_session', turnCount: 3, includesSubagents: false },
  coverage: { level: 'complete', available: [], missing: [], notes: [] },
  time: { accounting: 'approximate', denominator: 'session_wall_time', wallTimeMs: 1000 },
  tokens: {
    source: 'token_usage_records',
    inputTokens: 10,
    outputTokens: 5,
    totalTokens: 15,
    cacheCoverage: 'unavailable',
  },
  models: [],
  tools: [],
  files: { scope: 'unavailable', files: [] },
  compression: { compactionCount: 0, manualCompactionCount: 0, automaticCompactionCount: 0 },
  errors: { totalErrors: 0, toolErrors: 0, modelErrors: 0, examples: [] },
  slowest: [],
  privacy: {
    promptContentIncluded: false,
    toolInputsIncluded: false,
    commandOutputsIncluded: false,
    fileContentsIncluded: false,
    redactedFields: [],
  },
});

describe('SessionUsageModal', () => {
  let container: HTMLDivElement;
  let root: Root;

  function showReport() {
    act(() => {
      openSessionUsageModal({ sessionId: 'session-1', workspacePath: 'D:/workspace/OpenBitFun' });
      showSessionUsageModalReport({
        sessionId: 'session-1',
        report: report(),
        markdown: '# Session Usage Report',
      });
    });
  }

  beforeEach(() => {
    mocks.openSessionUsagePanel.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => root.render(<SessionUsageModal />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    closeSessionUsageModal();
  });

  it('renders nothing until there is a report to show', () => {
    expect(document.querySelector('[data-testid="session-usage-modal"]')).toBeNull();
    expect(isChatPopupActive()).toBe(false);
  });

  it('takes Escape away from the container while it is open', () => {
    /*
     * The container disables its own Escape shortcut while a chat popup is
     * active. Without this latch, dismissing a report would also cancel
     * whatever the session is running.
     */
    showReport();
    expect(isChatPopupActive()).toBe(true);

    act(() => closeSessionUsageModal());
    expect(isChatPopupActive()).toBe(false);
  });

  it('gives Escape back when it goes away with the report still open', () => {
    showReport();
    act(() => root.unmount());
    expect(isChatPopupActive()).toBe(false);
    // The afterEach unmount would otherwise run against a torn-down root.
    root = createRoot(container);
  });

  it('hands over to the panel rather than competing with it', async () => {
    showReport();
    const details = document.querySelector<HTMLButtonElement>(
      'button[aria-label="usage.actions.openDetails"]',
    );
    expect(details).not.toBeNull();

    await act(async () => {
      details?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      // The panel module is imported on demand.
      await Promise.resolve();
    });

    expect(mocks.openSessionUsagePanel).toHaveBeenCalledWith(expect.objectContaining({
      sessionId: 'session-1',
      workspacePath: 'D:/workspace/OpenBitFun',
      markdown: '# Session Usage Report',
      expand: true,
    }));
    expect(getSessionUsageModalState().open).toBe(false);
  });
});
