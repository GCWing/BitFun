import { describe, expect, it } from 'vitest';
import enFlowChat from '@/locales/en-US/flow-chat.json';
import zhCnFlowChat from '@/locales/zh-CN/flow-chat.json';
import zhTwFlowChat from '@/locales/zh-TW/flow-chat.json';
import enAgents from '@/locales/en-US/scenes/agents.json';
import zhCnAgents from '@/locales/zh-CN/scenes/agents.json';
import zhTwAgents from '@/locales/zh-TW/scenes/agents.json';

const LOCALES = {
  'en-US': enFlowChat,
  'zh-CN': zhCnFlowChat,
  'zh-TW': zhTwFlowChat,
};

const AGENT_LOCALES = {
  'en-US': enAgents,
  'zh-CN': zhCnAgents,
  'zh-TW': zhTwAgents,
};

const REQUIRED_ACTION_BAR_KEYS = [
  'deepReviewActionBar.minimize',
  'deepReviewActionBar.restore',
  'deepReviewActionBar.reviewRunningDeep',
  'deepReviewActionBar.reviewRunningStandard',
  'deepReviewActionBar.fixAndReviewRunning',
  'deepReviewActionBar.minimizedDeep',
  'deepReviewActionBar.minimizedStandard',
  'deepReviewActionBar.minimizedReviewRunningDeep',
  'deepReviewActionBar.minimizedReviewRunningStandard',
  'deepReviewActionBar.minimizedFix',
  'deepReviewActionBar.minimizedFixReview',
  'deepReviewActionBar.minimizedFixCompleted',
  'deepReviewActionBar.minimizedFixFailed',
  'deepReviewActionBar.minimizedReviewInterrupted',
  'deepReviewActionBar.minimizedResume',
  'deepReviewActionBar.fixInterrupted',
  'deepReviewActionBar.continueFix',
  'deepReviewActionBar.skipRemaining',
  'deepReviewActionBar.manualCancel.title',
  'deepReviewActionBar.manualCancel.description',
  'deepReviewActionBar.decisionGate.title',
  'deepReviewActionBar.decisionGate.description',
  'deepReviewActionBar.decisionGate.supplementLabel',
  'deepReviewActionBar.decisionGate.supplementPlaceholder',
  'deepReviewActionBar.decisionGate.missingSelection',
  'deepReviewActionBar.decisionGate.noOptionsHint',
  'deepReviewActionBar.decisionGate.confirmFix',
  'deepReviewActionBar.decisionGate.confirmFixAndReview',
  'deepReviewActionBar.decisionGate.cancel',
  'deepReviewActionBar.switchModel',
  'deepReviewActionBar.capacityQueue.title',
  'deepReviewActionBar.capacityQueue.pausedTitle',
  'deepReviewActionBar.capacityQueue.detail',
  'deepReviewActionBar.capacityQueue.sessionBusy',
  'deepReviewActionBar.capacityQueue.waitingReviewersTitle',
  'deepReviewActionBar.capacityQueue.reviewerStatusQueued',
  'deepReviewActionBar.capacityQueue.reviewerStatusPaused',
  'deepReviewActionBar.capacityQueue.optionalReviewer',
  'deepReviewActionBar.capacityQueue.pauseQueue',
  'deepReviewActionBar.capacityQueue.continueQueue',
  'deepReviewActionBar.capacityQueue.cancelQueued',
  'deepReviewActionBar.capacityQueue.skipOptionalQueued',
  'deepReviewActionBar.capacityQueue.openReviewSettings',
  'deepReviewActionBar.capacityQueue.reasons.launchBatchBlocked',
  'deepReviewActionBar.capacityQueue.reasonDetails.providerRateLimit',
  'deepReviewActionBar.capacityQueue.reasonDetails.providerConcurrencyLimit',
  'deepReviewActionBar.capacityQueue.reasonDetails.retryAfter',
  'deepReviewActionBar.capacityQueue.reasonDetails.localConcurrencyCap',
  'deepReviewActionBar.capacityQueue.reasonDetails.launchBatchBlocked',
  'deepReviewActionBar.capacityQueue.reasonDetails.temporaryOverload',
  'deepReviewActionBar.capacityQueue.controlFailed',
  'deepReviewActionBar.capacityQueue.controlFailedWithReason',
  'deepReviewActionBar.capacityQueue.controlPartiallyFailedWithReason',
  'reviewActionBar.noIssuesFound',
];

const REQUIRED_CODE_REVIEW_CARD_KEYS = [
  'toolCards.codeReview.noIssues',
  'toolCards.codeReview.severities.critical',
  'toolCards.codeReview.severities.high',
  'toolCards.codeReview.severities.medium',
  'toolCards.codeReview.severities.low',
  'toolCards.codeReview.severities.info',
  'toolCards.codeReview.reviewerStatuses.completed',
  'toolCards.codeReview.reviewerStatuses.timed_out',
  'toolCards.codeReview.reviewerStatuses.cancelled_by_user',
  'toolCards.codeReview.reviewerStatuses.cancelled',
  'toolCards.codeReview.reviewerStatuses.failed',
  'toolCards.codeReview.reviewerStatuses.skipped',
  'toolCards.codeReview.reviewerStatuses.running',
  'toolCards.codeReview.reviewerStatuses.partial',
  'toolCards.codeReview.reviewerStatuses.unknown',
];

const USER_VISIBLE_TEXT_KEYS_MUST_NOT_CONTAIN_ESCAPED_UNICODE = [
  'toolCards.codeReview.remediationActions.fixAndReview',
];

const REQUIRED_REVIEW_TEAM_PAGE_KEYS = [
  'reviewTeams.detail.loading',
];

const REQUIRED_DEEP_REVIEW_CONSENT_KEYS = [
  'deepReviewConsent.sessionConcurrencyTitle',
  'deepReviewConsent.sessionConcurrencyBody',
];

function getMessageValue(messages: unknown, key: string): unknown {
  return key
    .split('.')
    .reduce<unknown>((current, part) => {
      if (!current || typeof current !== 'object') {
        return undefined;
      }
      return (current as Record<string, unknown>)[part];
    }, messages);
}

describe('DeepReviewActionBar i18n', () => {
  it('keeps action bar chrome strings available in every bundled locale', () => {
    for (const [locale, messages] of Object.entries(LOCALES)) {
      const missingKeys = REQUIRED_ACTION_BAR_KEYS.filter((key) => {
        const value = getMessageValue(messages, key);
        return typeof value !== 'string' || value.trim().length === 0;
      });

      expect(missingKeys, `${locale} missing keys`).toEqual([]);
    }
  });

  it('keeps review report lineup strings available in every bundled locale', () => {
    for (const [locale, messages] of Object.entries(LOCALES)) {
      const missingKeys = REQUIRED_CODE_REVIEW_CARD_KEYS.filter((key) => {
        const value = getMessageValue(messages, key);
        return typeof value !== 'string' || value.trim().length === 0;
      });

      expect(missingKeys, `${locale} missing keys`).toEqual([]);
    }
  });

  it('does not show escaped unicode sequences in user-visible action text', () => {
    for (const [locale, messages] of Object.entries(LOCALES)) {
      for (const key of USER_VISIBLE_TEXT_KEYS_MUST_NOT_CONTAIN_ESCAPED_UNICODE) {
        const value = getMessageValue(messages, key);
        expect(typeof value, `${locale} ${key} should be a string`).toBe('string');
        expect(value, `${locale} ${key} should not contain literal unicode escape text`).not.toMatch(/\\u[0-9a-fA-F]{4}/);
      }
    }
  });

  it('keeps review team page strings available in every bundled locale', () => {
    for (const [locale, messages] of Object.entries(AGENT_LOCALES)) {
      const missingKeys = REQUIRED_REVIEW_TEAM_PAGE_KEYS.filter((key) => {
        const value = getMessageValue(messages, key);
        return typeof value !== 'string' || value.trim().length === 0;
      });

      expect(missingKeys, `${locale} missing keys`).toEqual([]);
    }
  });

  it('keeps Deep Review consent strings available in every bundled locale', () => {
    for (const [locale, messages] of Object.entries(LOCALES)) {
      const missingKeys = REQUIRED_DEEP_REVIEW_CONSENT_KEYS.filter((key) => {
        const value = getMessageValue(messages, key);
        return typeof value !== 'string' || value.trim().length === 0;
      });

      expect(missingKeys, `${locale} missing keys`).toEqual([]);
    }
  });
});
