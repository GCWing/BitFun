export type ReviewCoverageSourceLabelKey =
  | 'businessLogic'
  | 'performance'
  | 'security'
  | 'architecture'
  | 'frontend'
  | 'focusedCheck'
  | 'qualityGate';

export const DEFAULT_REVIEW_COVERAGE_SOURCE_LABELS: Record<
  ReviewCoverageSourceLabelKey,
  string
> = {
  businessLogic: 'Logic coverage',
  performance: 'Performance coverage',
  security: 'Security coverage',
  architecture: 'Architecture coverage',
  frontend: 'Frontend coverage',
  focusedCheck: 'Additional check',
  qualityGate: 'Quality check',
};

const REVIEW_SOURCE_ALIASES: Record<string, ReviewCoverageSourceLabelKey> = {
  reviewworker: 'focusedCheck',
  reviewbusinesslogic: 'businessLogic',
  logicreviewer: 'businessLogic',
  businesslogicreviewer: 'businessLogic',
  reviewperformance: 'performance',
  performancereviewer: 'performance',
  reviewsecurity: 'security',
  securityreviewer: 'security',
  reviewarchitecture: 'architecture',
  architecturereviewer: 'architecture',
  reviewfrontend: 'frontend',
  frontendreviewer: 'frontend',
  reviewjudge: 'qualityGate',
  reviewarbiter: 'qualityGate',
  reviewqualityinspector: 'qualityGate',
  reviewqualitycheck: 'qualityGate',
  qualityinspector: 'qualityGate',
};

function normalizeReviewSource(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/g, '');
}

export function resolveReviewCoverageSourceLabelKey(
  sourceReviewer?: string | null,
): ReviewCoverageSourceLabelKey | null {
  const normalized = sourceReviewer?.trim();
  if (!normalized) {
    return null;
  }

  return REVIEW_SOURCE_ALIASES[normalizeReviewSource(normalized)] ?? null;
}

export function formatReviewCoverageSource(
  sourceReviewer: string | undefined,
  labels: Record<ReviewCoverageSourceLabelKey, string> = DEFAULT_REVIEW_COVERAGE_SOURCE_LABELS,
): string | null {
  const normalized = sourceReviewer?.trim();
  if (!normalized) {
    return null;
  }

  const labelKey = resolveReviewCoverageSourceLabelKey(normalized);
  if (labelKey) {
    return labels[labelKey];
  }

  // Keep the known remediation identity, but never project arbitrary model
  // prose, Skill names, or custom-agent names into the product report. Only
  // the structured ReviewWorker runtime identity proves an additional check ran.
  return normalizeReviewSource(normalized) === 'reviewfixer'
    ? normalized
    : null;
}
