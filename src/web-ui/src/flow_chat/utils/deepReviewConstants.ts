/**
 * Shared constants for Deep Review feature to avoid duplication across modules.
 */

/** Canonical typed command for strict Review. Historical DeepReview forms are compatibility aliases. */
export const REVIEW_STRICT_SLASH_COMMAND = '/review strict';
export const DEEP_REVIEW_SLASH_COMMAND_ALIAS = '/DeepReview';
export const DEEP_REVIEW_COMMAND_RE = /^\/(?:review\s+strict|DeepReview|deepreview)(?:\s+.*)?$/;
export const REVIEW_STRICT_COMMAND_PREFIX_RE = /^\/review\s+strict\b/i;
export const DEEP_REVIEW_COMPAT_COMMAND_PREFIX_RE = /^\/(?:DeepReview|deepreview)\b/;
