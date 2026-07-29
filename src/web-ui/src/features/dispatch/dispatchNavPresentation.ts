import type { DispatchJobState, DispatchReachability } from './types';

interface DispatchNavPresentationInput {
  targetLabel: string;
  state: DispatchJobState;
  reachability?: DispatchReachability;
  runningSummary: string;
  unreachableLabel: string;
  unreachableSummary: string;
}

export interface DispatchNavPresentation {
  badgeLabel: string;
  summary: string;
  visualState: DispatchJobState | 'unreachable';
}

/**
 * Transport reachability is presentation-only. It can override the badge
 * treatment while leaving the target's authoritative job state untouched.
 */
export function resolveDispatchNavPresentation(
  input: DispatchNavPresentationInput,
): DispatchNavPresentation {
  if (input.reachability === 'unreachable') {
    return {
      badgeLabel: input.unreachableLabel,
      summary: input.unreachableSummary,
      visualState: 'unreachable',
    };
  }
  return {
    badgeLabel: input.targetLabel,
    summary: input.runningSummary,
    visualState: input.state,
  };
}
