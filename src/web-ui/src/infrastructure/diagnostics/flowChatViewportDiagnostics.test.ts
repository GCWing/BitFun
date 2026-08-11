// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const diagnosticsMocks = vi.hoisted(() => ({
  trace: vi.fn(),
  isEnabled: vi.fn(() => true),
}));

vi.mock('./flowChatDiagnostics', () => ({
  flowChatDiagnostics: { trace: diagnosticsMocks.trace },
  isFlowChatDiagnosticsEnabled: diagnosticsMocks.isEnabled,
}));

import {
  coalesceViewportRepeat,
  FLOWCHAT_VIEWPORT_PROBE,
  resetViewportDiagnosticsForTests,
  traceViewportPlacement,
  traceViewportRepeating,
  VIEWPORT_TRACE_REPEAT_INTERVAL_MS,
} from './flowChatViewportDiagnostics';

describe('coalesceViewportRepeat', () => {
  it('emits the first sample of a run, which is the transition', () => {
    const decision = coalesceViewportRepeat(null, { nowMs: 1_000 });

    expect(decision.emit).toEqual({
      suppressedCount: 0,
      suppressedTravelPx: 0,
      suppressedForMs: 0,
    });
  });

  it('folds samples inside the interval into the next emission', () => {
    let state = coalesceViewportRepeat(null, { nowMs: 0 }).state;
    for (let frame = 1; frame <= 12; frame += 1) {
      const decision = coalesceViewportRepeat(state, { nowMs: frame * 16, travelPx: -2 });
      expect(decision.emit).toBeNull();
      state = decision.state;
    }

    const emitted = coalesceViewportRepeat(state, {
      nowMs: VIEWPORT_TRACE_REPEAT_INTERVAL_MS,
      travelPx: -2,
    });

    expect(emitted.emit).toEqual({
      suppressedCount: 12,
      // Travel is accumulated as distance, so a run that never nets anywhere is
      // still visible as movement.
      suppressedTravelPx: 24,
      suppressedForMs: VIEWPORT_TRACE_REPEAT_INTERVAL_MS - 16,
    });
  });

  it('starts the suppression clock at the first suppressed sample, not the last emission', () => {
    const first = coalesceViewportRepeat(null, { nowMs: 0 });
    const suppressed = coalesceViewportRepeat(first.state, { nowMs: 400 });

    expect(suppressed.state.firstSuppressedAtMs).toBe(400);
    expect(coalesceViewportRepeat(suppressed.state, { nowMs: 500 }).emit).toMatchObject({
      suppressedCount: 1,
      suppressedForMs: 100,
    });
  });

  it('reports nothing suppressed for a run that resumes after a quiet interval', () => {
    const first = coalesceViewportRepeat(null, { nowMs: 0 });

    expect(coalesceViewportRepeat(first.state, { nowMs: 5_000 }).emit).toEqual({
      suppressedCount: 0,
      suppressedTravelPx: 0,
      suppressedForMs: 0,
    });
  });
});

describe('traceViewportRepeating', () => {
  beforeEach(() => {
    diagnosticsMocks.trace.mockReset();
    diagnosticsMocks.isEnabled.mockReturnValue(true);
    resetViewportDiagnosticsForTests();
  });

  it('emits once per key per interval and carries what it stands for', () => {
    const nowSpy = vi.spyOn(performance, 'now');
    nowSpy.mockReturnValue(0);
    traceViewportRepeating('write|follow-output|granted', {
      location: 'viewportOwner.write',
      message: 'wrote',
      travelPx: 10,
    });
    nowSpy.mockReturnValue(16);
    traceViewportRepeating('write|follow-output|granted', {
      location: 'viewportOwner.write',
      message: 'wrote',
      travelPx: 10,
    });

    expect(diagnosticsMocks.trace).toHaveBeenCalledTimes(1);

    nowSpy.mockReturnValue(600);
    traceViewportRepeating('write|follow-output|granted', {
      location: 'viewportOwner.write',
      message: 'wrote',
      travelPx: 10,
      data: () => ({ owner: 'follow-output' }),
    });

    expect(diagnosticsMocks.trace).toHaveBeenCalledTimes(2);
    const probe = diagnosticsMocks.trace.mock.calls[1][0];
    expect(probe.hypothesis).toBe(FLOWCHAT_VIEWPORT_PROBE);
    expect(probe.data()).toEqual({
      owner: 'follow-output',
      repeated: { suppressedCount: 1, suppressedTravelPx: 10, suppressedForMs: 584 },
    });
    nowSpy.mockRestore();
  });

  it('keeps interleaved runs apart, so a change of outcome is never buried', () => {
    traceViewportRepeating('write|follow-output|granted', {
      location: 'viewportOwner.write',
      message: 'wrote',
    });
    traceViewportRepeating('write|anchor-correction|refused', {
      location: 'viewportOwner.write',
      message: 'refused',
    });

    expect(diagnosticsMocks.trace).toHaveBeenCalledTimes(2);
  });

  it('evaluates nothing while diagnostics are off', () => {
    diagnosticsMocks.isEnabled.mockReturnValue(false);
    const data = vi.fn(() => ({ scrollTop: 1 }));

    traceViewportRepeating('write|follow-output|granted', {
      location: 'viewportOwner.write',
      message: 'wrote',
      data,
    });

    expect(data).not.toHaveBeenCalled();
    expect(diagnosticsMocks.trace).not.toHaveBeenCalled();
  });
});

describe('traceViewportPlacement', () => {
  beforeEach(() => {
    diagnosticsMocks.trace.mockReset();
    diagnosticsMocks.isEnabled.mockReturnValue(true);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('reports the drift a later writer introduced', () => {
    const scroller = document.createElement('div');
    Object.defineProperty(scroller, 'scrollTop', { value: 0, writable: true });

    traceViewportPlacement(
      scroller,
      { location: 'visibleTask.scrollToTask', message: 'placed', targetPx: 900 },
      () => {
        scroller.scrollTop = 900;
      },
    );
    // Something else took the viewport back after the placement landed.
    scroller.scrollTop = 120;
    vi.runAllTimers();

    expect(diagnosticsMocks.trace).toHaveBeenCalledTimes(2);
    expect(diagnosticsMocks.trace.mock.calls[0][0].data()).toMatchObject({
      beforePx: 0,
      placedPx: 900,
      targetPx: 900,
    });
    expect(diagnosticsMocks.trace.mock.calls[1][0].location)
      .toBe('visibleTask.scrollToTask.outcome');
    expect(diagnosticsMocks.trace.mock.calls[1][0].data()).toMatchObject({
      settledPx: 120,
      driftPx: -780,
    });
  });

  it('still performs the placement while diagnostics are off', () => {
    diagnosticsMocks.isEnabled.mockReturnValue(false);
    const place = vi.fn();

    traceViewportPlacement(
      document.createElement('div'),
      { location: 'navigation.scrollIntoView', message: 'placed' },
      place,
    );
    vi.runAllTimers();

    expect(place).toHaveBeenCalledTimes(1);
    expect(diagnosticsMocks.trace).not.toHaveBeenCalled();
  });
});
