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
    'Member-level strategy overrides take precedence over the team strategy.',
    'The reviewer Task prompt must include the resolved prompt_directive.',
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
      'Fast screening for high-confidence issues in the requested diff or scope.',
    tokenImpact: '0.4-0.6x',
    runtimeImpact: '0.5-0.7x',
    defaultModelSlot: 'fast',
    promptDirective:
      'Prefer a concise diff-focused pass. Report only high-confidence correctness, security, or regression risks and avoid speculative design rewrites.',
    roleDirectives: {
      ReviewBusinessLogic:
        'Only trace logic paths directly changed by the diff. Do not follow call chains beyond one hop. Report only issues where the diff introduces a provably wrong behavior.',
      ReviewPerformance:
        'Scan the diff for known anti-patterns only: nested loops, repeated fetches, blocking calls on hot paths, unnecessary re-renders. Do not trace call chains or estimate impact beyond what the diff shows.',
      ReviewSecurity:
        'Scan the diff for direct security risks only: injection, secret exposure, unsafe commands, missing auth. Do not trace data flows beyond one hop.',
      ReviewArchitecture:
        'Only check imports directly changed by the diff. Flag violations of documented layer boundaries.',
      ReviewFrontend:
        'Only check i18n key completeness and direct platform boundary violations in changed frontend files.',
      ReviewJudge:
        'This was a quick review. Focus on confirming or rejecting each finding efficiently. If a finding\'s evidence is thin, reject it rather than spending time verifying.',
    },
  },
  normal: {
    level: 'normal',
    label: 'Normal',
    summary:
      'Balanced review depth for day-to-day code review with practical evidence.',
    tokenImpact: '1x',
    runtimeImpact: '1x',
    defaultModelSlot: 'fast',
    promptDirective:
      'Perform the standard role-specific review. Balance coverage with precision and include concrete evidence for each issue.',
    roleDirectives: {
      ReviewBusinessLogic:
        'Trace each changed function\'s direct callers and callees to verify business rules and state transitions. Stop investigating a path once you have enough evidence to confirm or dismiss it.',
      ReviewPerformance:
        'Inspect the diff for anti-patterns, then read surrounding code to confirm impact on hot paths. Report only issues likely to matter at realistic scale.',
      ReviewSecurity:
        'Trace each changed input path from entry point to usage. Check trust boundaries, auth assumptions, and data sanitization. Report only issues with a realistic threat narrative.',
      ReviewArchitecture:
        "Check the diff's imports plus one level of dependency direction. Verify API contract consistency.",
      ReviewFrontend:
        'Check i18n, React performance patterns, and accessibility in changed components. Verify frontend-backend API contract alignment.',
      ReviewJudge:
        'Validate each finding\'s logical consistency and evidence quality. Spot-check code only when a claim needs verification.',
    },
  },
  deep: {
    level: 'deep',
    label: 'Deep',
    summary:
      'Thorough multi-pass review for risky, broad, or release-sensitive changes.',
    tokenImpact: '1.8-2.5x',
    runtimeImpact: '1.5-2.5x',
    defaultModelSlot: 'primary',
    promptDirective:
      'Run a thorough role-specific pass. Inspect edge cases, cross-file interactions, failure modes, and remediation tradeoffs before finalizing findings.',
    roleDirectives: {
      ReviewBusinessLogic:
        'Map full call chains for changed functions. Verify state transitions end-to-end, check rollback and error-recovery paths, and test edge cases in data shape and lifecycle assumptions. Prioritize findings by user-facing impact.',
      ReviewPerformance:
        'In addition to the normal pass, check for latent scaling risks — data structures that degrade at volume, or algorithms that are correct but unnecessarily expensive. Only report if you can estimate the impact. Do not speculate about edge cases or failure modes unrelated to performance.',
      ReviewSecurity:
        'In addition to the normal pass, trace data flows across trust boundaries end-to-end. Check for privilege escalation chains, indirect injection vectors, and failure modes that expose sensitive data. Report only issues with a complete threat narrative.',
      ReviewArchitecture:
        'Map the full dependency graph for changed modules. Check for structural anti-patterns, circular dependencies, and cross-cutting concerns.',
      ReviewFrontend:
        'Thorough React analysis: effect dependencies, memoization, virtualization. Full accessibility audit. State management pattern review. Cross-layer contract verification.',
      ReviewJudge:
        'This was a deep review with potentially complex findings. Cross-validate findings across reviewers for consistency. For each finding, verify the evidence supports the conclusion and the suggested fix is safe. Pay extra attention to overlapping findings across reviewers or same-role instances.',
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
