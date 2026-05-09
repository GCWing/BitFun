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
  'deepReviewActionBar.fixAndReviewRunning',
  'deepReviewActionBar.minimizedDeep',
  'deepReviewActionBar.minimizedStandard',
  'deepReviewActionBar.minimizedFix',
  'deepReviewActionBar.minimizedFixReview',
  'deepReviewActionBar.minimizedFixCompleted',
  'deepReviewActionBar.minimizedFixFailed',
  'deepReviewActionBar.minimizedReviewInterrupted',
  'deepReviewActionBar.minimizedResume',
  'deepReviewActionBar.fixInterrupted',
  'deepReviewActionBar.continueFix',
  'deepReviewActionBar.skipRemaining',
  'deepReviewActionBar.switchModel',
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

const REQUIRED_REVIEW_TEAM_PAGE_KEYS = [
  'reviewTeams.detail.loading',
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

  it('keeps review team page strings available in every bundled locale', () => {
    for (const [locale, messages] of Object.entries(AGENT_LOCALES)) {
      const missingKeys = REQUIRED_REVIEW_TEAM_PAGE_KEYS.filter((key) => {
        const value = getMessageValue(messages, key);
        return typeof value !== 'string' || value.trim().length === 0;
      });

      expect(missingKeys, `${locale} missing keys`).toEqual([]);
    }
  });
});
