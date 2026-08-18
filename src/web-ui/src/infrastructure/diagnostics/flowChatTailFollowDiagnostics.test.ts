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
  accumulateTailFollowStep,
  createTailFollowWindow,
  FLOWCHAT_TAIL_FOLLOW_PROBE,
  noteFlowListCommit,
  noteTailFollowStep,
  resetTailFollowDiagnosticsForTests,
  summariseTailFollowWindow,
  tailFollowStepBucket,
  TAIL_FOLLOW_WINDOW_MS,
  type TailFollowStepSample,
} from './flowChatTailFollowDiagnostics';

function step(overrides: Partial<TailFollowStepSample> = {}): TailFollowStepSample {
  return { stepPx: 24, lagPx: 24, innerScroll: true, snapped: false, ...overrides };
}

describe('tailFollowStepBucket', () => {
  it('puts a whole line in the bucket that means "no better than not smoothing"', () => {
    expect(tailFollowStepBucket(24)).toBe('to28');
  });

  it('separates a genuine improvement from an imperceptible one', () => {
    expect(tailFollowStepBucket(3.9)).toBe('under4');
    expect(tailFollowStepBucket(4)).toBe('to12');
    expect(tailFollowStepBucket(11.9)).toBe('to12');
  });

  it('flags a step worse than a line', () => {
    expect(tailFollowStepBucket(28)).toBe('over28');
    expect(tailFollowStepBucket(120)).toBe('over28');
  });

  it('buckets by magnitude, so a correction upwards is not a separate story', () => {
    expect(tailFollowStepBucket(-24)).toBe('to28');
  });
});

describe('accumulateTailFollowStep', () => {
  it('keeps the peaks, which is what a mean would hide', () => {
    let window = createTailFollowWindow(0, 0);
    window = accumulateTailFollowStep(window, step({ stepPx: 2, lagPx: 2 }));
    window = accumulateTailFollowStep(window, step({ stepPx: 96, lagPx: 140 }));
    window = accumulateTailFollowStep(window, step({ stepPx: 2, lagPx: 2 }));

    expect(window.maxStepPx).toBe(96);
    expect(window.maxLagPx).toBe(140);
    expect(window.travelPx).toBe(100);
    expect(window.buckets).toEqual({ under4: 2, to12: 0, to28: 0, over28: 1 });
  });

  it('counts snaps and inner-scroll steps apart', () => {
    let window = createTailFollowWindow(0, 0);
    window = accumulateTailFollowStep(window, step({ snapped: true, innerScroll: false }));
    window = accumulateTailFollowStep(window, step({ snapped: false, innerScroll: true }));

    expect(window.snaps).toBe(1);
    expect(window.innerScrollSteps).toBe(1);
  });

  it('does not mutate the window it is given', () => {
    const window = createTailFollowWindow(0, 0);
    accumulateTailFollowStep(window, step());

    expect(window.steps).toBe(0);
    expect(window.buckets.to28).toBe(0);
  });
});

describe('summariseTailFollowWindow', () => {
  it('reports list commits as the delta over the window, not the running total', () => {
    const window = createTailFollowWindow(0, 40);
    const summary = summariseTailFollowWindow(window, 1_000, 55);

    expect(summary.listCommits).toBe(15);
    expect(summary.listCommitsPerSec).toBe(15);
  });

  it('scales a short trailing window up so it compares with the full ones', () => {
    let window = createTailFollowWindow(0, 0);
    for (let index = 0; index < 5; index += 1) {
      window = accumulateTailFollowStep(window, step());
    }
    const summary = summariseTailFollowWindow(window, 250, 0);

    expect(summary.forMs).toBe(250);
    expect(summary.steps).toBe(5);
    expect(summary.stepsPerSec).toBe(20);
  });

  it('reports no average step for a window that recorded none', () => {
    const summary = summariseTailFollowWindow(createTailFollowWindow(0, 0), 1_000, 0);

    expect(summary.avgStepPx).toBe(0);
  });
});

describe('noteTailFollowStep', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    diagnosticsMocks.trace.mockClear();
    diagnosticsMocks.isEnabled.mockReturnValue(true);
    resetTailFollowDiagnosticsForTests();
  });

  afterEach(() => {
    resetTailFollowDiagnosticsForTests();
    vi.useRealTimers();
  });

  it('records nothing while the diagnostics switch is off', () => {
    diagnosticsMocks.isEnabled.mockReturnValue(false);

    noteTailFollowStep('thinking', step());
    vi.advanceTimersByTime(TAIL_FOLLOW_WINDOW_MS * 3);

    expect(diagnosticsMocks.trace).not.toHaveBeenCalled();
  });

  it('summarises a run of frames into one entry rather than one per frame', () => {
    for (let frame = 0; frame < 40; frame += 1) {
      noteTailFollowStep('thinking', step({ stepPx: 6, lagPx: 18 }));
      vi.advanceTimersByTime(16);
    }
    vi.advanceTimersByTime(TAIL_FOLLOW_WINDOW_MS * 2);

    const entries = diagnosticsMocks.trace.mock.calls.map(([probe]) => probe);
    expect(entries.length).toBeGreaterThan(0);
    expect(entries.length).toBeLessThan(40);
    expect(entries[0].hypothesis).toBe(FLOWCHAT_TAIL_FOLLOW_PROBE);
    expect(entries[0].location).toBe('tailFollow.thinking');
    const total = entries.reduce((sum, probe) => sum + probe.data().steps, 0);
    expect(total).toBe(40);
  });

  it('closes out the last window of a stream, which nothing else would report', () => {
    noteTailFollowStep('thinking', step());
    expect(diagnosticsMocks.trace).not.toHaveBeenCalled();

    vi.advanceTimersByTime(TAIL_FOLLOW_WINDOW_MS * 2);

    expect(diagnosticsMocks.trace).toHaveBeenCalledTimes(1);
    expect(diagnosticsMocks.trace.mock.calls[0][0].data().steps).toBe(1);
  });

  it('attributes only the list commits that happened inside the window', () => {
    noteFlowListCommit();
    noteFlowListCommit();
    noteTailFollowStep('thinking', step());
    noteFlowListCommit();
    vi.advanceTimersByTime(TAIL_FOLLOW_WINDOW_MS * 2);

    expect(diagnosticsMocks.trace.mock.calls[0][0].data().listCommits).toBe(1);
  });

  it('ignores list commits while the switch is off, so the count is not backdated', () => {
    diagnosticsMocks.isEnabled.mockReturnValue(false);
    noteFlowListCommit();
    diagnosticsMocks.isEnabled.mockReturnValue(true);

    noteTailFollowStep('thinking', step());
    noteFlowListCommit();
    vi.advanceTimersByTime(TAIL_FOLLOW_WINDOW_MS * 2);

    expect(diagnosticsMocks.trace.mock.calls[0][0].data().listCommits).toBe(1);
  });
});
