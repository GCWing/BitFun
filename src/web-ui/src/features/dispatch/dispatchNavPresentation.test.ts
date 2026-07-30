import { describe, expect, it } from 'vitest';
import { resolveDispatchNavPresentation } from './dispatchNavPresentation';

describe('resolveDispatchNavPresentation', () => {
  it('shows target unreachability in both the navigation badge and tooltip summary', () => {
    const presentation = resolveDispatchNavPresentation({
      targetLabel: 'build-host',
      state: 'running',
      reachability: 'unreachable',
      runningSummary: 'Runs on build-host · running',
      unreachableLabel: 'Target unreachable',
      unreachableSummary: 'Target unreachable: build-host · SSH target is offline',
    });

    expect(presentation).toEqual({
      badgeLabel: 'Target unreachable',
      summary: 'Target unreachable: build-host · SSH target is offline',
      visualState: 'unreachable',
    });
  });

  it('keeps the authoritative job state presentation after transport recovery', () => {
    const presentation = resolveDispatchNavPresentation({
      targetLabel: 'build-host',
      state: 'running',
      reachability: 'reachable',
      runningSummary: 'Runs on build-host · running',
      unreachableLabel: 'Target unreachable',
      unreachableSummary: 'Target unreachable: build-host · stale error',
    });

    expect(presentation).toEqual({
      badgeLabel: 'build-host',
      summary: 'Runs on build-host · running',
      visualState: 'running',
    });
  });
});
