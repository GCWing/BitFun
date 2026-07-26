import type {
  ReviewStrategyCommonRules,
  ReviewStrategyLevel,
  ReviewStrategyProfile,
} from './types';

export const REVIEW_STRATEGY_LEVELS: ReviewStrategyLevel[] = [
  'quick',
  'normal',
  'deep',
];

export const REVIEW_STRATEGY_COMMON_RULES: ReviewStrategyCommonRules = {
  reviewerPromptRules: [
    'Each reviewer must follow its own strategy field.',
    'Reviewer-level strategy overrides take precedence over the review strategy.',
    'The reviewer LaunchReviewAgent prompt must include the resolved prompt_directive.',
  ],
};

export const REVIEW_STRATEGY_PROFILES: Record<
  ReviewStrategyLevel,
  ReviewStrategyProfile
> = {
  quick: {
    level: 'quick',
    label: 'Quick',
    summary:
      'Quick keeps the main review concise and allows narrowly focused extra checks only when justified.',
    defaultModelSlot: 'fast',
    promptDirective:
      'Prefer a concise diff-focused pass. Report only high-confidence correctness, security, or regression risks and avoid speculative design rewrites.',
    roleDirectives: {
      ReviewWorker:
        'Answer only the supplied narrow question from direct diff evidence. Do not trace beyond one dependency hop.',
      ReviewJudge:
        'Confirm or reject the disputed finding efficiently; reject claims with thin evidence.',
    },
  },
  normal: {
    level: 'normal',
    label: 'Normal',
    summary:
      'Normal balances evidence depth with optional independent checks selected for the current change.',
    defaultModelSlot: 'fast',
    promptDirective:
      'Perform a practical evidence-backed review and stop investigating once each suspected issue is confirmed or dismissed.',
    roleDirectives: {
      ReviewWorker:
        'Apply the supplied lens to the changed path and its direct contracts. Report only realistic impact with concrete evidence.',
      ReviewJudge:
        'Validate each disputed finding and spot-check code only where its evidence needs verification.',
    },
  },
  deep: {
    level: 'deep',
    label: 'Deep',
    summary:
      'Deep gives the main review and any justified independent checks the longest bounded budget.',
    defaultModelSlot: 'primary',
    promptDirective:
      'Inspect edge cases, cross-file interactions, failure modes, and remediation tradeoffs before finalizing findings.',
    roleDirectives: {
      ReviewWorker:
        'Apply the supplied lens end-to-end within its exact scope, including relevant failure paths and cross-boundary contracts; do not broaden into unrelated review domains.',
      ReviewJudge:
        'Cross-check complex disputed findings and verify that both evidence and suggested remediation are safe.',
    },
  },
};

export const REVIEW_STRATEGY_DEFINITIONS = REVIEW_STRATEGY_PROFILES;
export type ReviewStrategyDefinition = ReviewStrategyProfile;

export function getReviewStrategyProfile(
  strategyLevel: ReviewStrategyLevel,
): ReviewStrategyProfile {
  return REVIEW_STRATEGY_PROFILES[strategyLevel];
}
