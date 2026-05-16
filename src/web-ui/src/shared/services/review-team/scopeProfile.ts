import type {
  DeepReviewRiskFocusTag,
  DeepReviewScopeProfile,
  ReviewStrategyLevel,
} from './types';

const DEEP_REVIEW_RISK_FOCUS_TAGS: DeepReviewRiskFocusTag[] = [
  'security',
  'data_loss',
  'migrations',
  'authentication_authorization',
  'cross_boundary_api_contracts',
  'concurrency',
  'persistence',
  'configuration_changes',
  'platform_boundary_violations',
];

export function buildDeepReviewScopeProfile(
  strategyLevel: ReviewStrategyLevel,
): DeepReviewScopeProfile {
  if (strategyLevel === 'quick') {
    return {
      reviewDepth: 'high_risk_only',
      riskFocusTags: [...DEEP_REVIEW_RISK_FOCUS_TAGS],
      maxDependencyHops: 0,
      optionalReviewerPolicy: 'risk_matched_only',
      allowBroadToolExploration: false,
      coverageExpectation:
        'High-risk-only pass. Keep all changed files visible in coverage metadata, but only report directly evidenced high-risk findings and do not claim full-depth coverage.',
    };
  }

  if (strategyLevel === 'normal') {
    return {
      reviewDepth: 'risk_expanded',
      riskFocusTags: [...DEEP_REVIEW_RISK_FOCUS_TAGS],
      maxDependencyHops: 1,
      optionalReviewerPolicy: 'configured',
      allowBroadToolExploration: false,
      coverageExpectation:
        'Risk-expanded pass. Cover changed files plus one-hop high-risk context when evidence requires it, and describe any reduced-depth confidence limits.',
    };
  }

  return {
    reviewDepth: 'full_depth',
    riskFocusTags: [...DEEP_REVIEW_RISK_FOCUS_TAGS],
    maxDependencyHops: 'policy_limited',
    optionalReviewerPolicy: 'full',
    allowBroadToolExploration: true,
    coverageExpectation:
      'Full-depth pass. Review changed files and policy-limited dependency context deeply enough to support release-quality findings.',
  };
}
