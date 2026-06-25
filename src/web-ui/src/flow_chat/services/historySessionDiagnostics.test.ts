import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  beginHistorySessionDiagnostics,
  recordHistorySessionDiagnosticEvent,
  resetHistorySessionDiagnosticsForTests,
  warnHistorySessionLoadingLayerStalled,
} from './historySessionDiagnostics';

const loggerMock = vi.hoisted(() => ({
  debug: vi.fn(),
  warn: vi.fn(),
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => loggerMock,
}));

describe('historySessionDiagnostics', () => {
  beforeEach(() => {
    loggerMock.debug.mockReset();
    loggerMock.warn.mockReset();
    resetHistorySessionDiagnosticsForTests();
  });

  it('logs a stalled loading layer as bounded grouped warnings once', () => {
    const diagnosticId = beginHistorySessionDiagnostics('history-1', 'open_intent', {
      source: 'pointerdown',
    });

    recordHistorySessionDiagnosticEvent('history-1', 'switch_requested', {
      switchRequestId: 1,
      shouldActivateBeforeHydrate: true,
    });
    recordHistorySessionDiagnosticEvent('history-1', 'hydrate_reused_pending', {
      pendingHydrateAgeMs: 125,
    });
    recordHistorySessionDiagnosticEvent('history-1', 'store_stale_commit_skipped', {
      activeSessionIdMatches: false,
    });

    warnHistorySessionLoadingLayerStalled('history-1', {
      durationMs: 800,
      historyState: 'metadata-only',
      isHistorical: true,
      isRemote: false,
      activeSessionIdMatches: true,
      hasRenderableContent: false,
      dialogTurnCount: 0,
    });
    warnHistorySessionLoadingLayerStalled('history-1', {
      durationMs: 1_200,
      historyState: 'metadata-only',
      isHistorical: true,
      isRemote: false,
      activeSessionIdMatches: true,
      hasRenderableContent: false,
      dialogTurnCount: 0,
    });

    expect(loggerMock.warn).toHaveBeenCalledTimes(3);
    expect(loggerMock.warn).toHaveBeenNthCalledWith(
      1,
      'Historical session loading layer stalled',
      expect.objectContaining({
        diagnosticId,
        sessionId: 'history-1',
        durationMs: 800,
        historyState: 'metadata-only',
        hasRenderableContent: false,
      }),
    );
    expect(loggerMock.warn).toHaveBeenNthCalledWith(
      2,
      'Historical session hydrate state at stall',
      expect.objectContaining({
        diagnosticId,
        sessionId: 'history-1',
        lastHydrateEvent: 'hydrate_reused_pending',
        lastStoreEvent: 'store_stale_commit_skipped',
      }),
    );
    expect(loggerMock.warn).toHaveBeenNthCalledWith(
      3,
      'Historical session recent lifecycle events',
      expect.objectContaining({
        diagnosticId,
        sessionId: 'history-1',
        events: expect.arrayContaining([
          expect.objectContaining({ event: 'switch_requested' }),
          expect.objectContaining({ event: 'hydrate_reused_pending' }),
          expect.objectContaining({ event: 'store_stale_commit_skipped' }),
        ]),
      }),
    );
  });
});
