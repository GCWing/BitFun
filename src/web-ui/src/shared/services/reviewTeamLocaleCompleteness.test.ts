import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { FALLBACK_REVIEW_TEAM_DEFINITION } from './reviewTeamService';

const REVIEW_TEAM_LOCALES = ['en-US', 'zh-CN', 'zh-TW'] as const;

type Locale = (typeof REVIEW_TEAM_LOCALES)[number];
type JsonObject = Record<string, unknown>;

const REVIEW_TEAM_FLOW_CHAT_KEYS = [
  'deepReviewConsent.runStrategy',
  'deepReviewConsent.recommendedStrategy',
  'deepReviewConsent.recommendationTitle',
  'deepReviewConsent.strategyOverrideTitle',
  'deepReviewConsent.strategyOverrideBody',
  'deepReviewConsent.teamDefaultStrategy',
  'deepReviewConsent.strategyLabels.quick',
  'deepReviewConsent.strategyLabels.normal',
  'deepReviewConsent.strategyLabels.deep',
  'toolCards.codeReview.runManifest.recommendedStrategy',
  'toolCards.codeReview.runManifest.riskRecommendationTitle',
] as const;

function readLocaleJson(
  locale: Locale,
  namespace: 'flow-chat.json' | 'scenes/agents.json' | 'settings/review.json',
) {
  const filePath = fileURLToPath(new URL(`../../locales/${locale}/${namespace}`, import.meta.url));
  return JSON.parse(readFileSync(filePath, 'utf8')) as JsonObject;
}

function getPathValue(source: JsonObject, path: string): unknown {
  return path.split('.').reduce<unknown>((current, segment) => {
    if (!current || typeof current !== 'object') {
      return undefined;
    }
    return (current as JsonObject)[segment];
  }, source);
}

function expectNonEmptyLocaleString(source: JsonObject, path: string) {
  const value = getPathValue(source, path);
  expect(value, path).toEqual(expect.any(String));
  expect((value as string).trim(), path).not.toBe('');
}

describe('review team locale completeness', () => {
  it.each(REVIEW_TEAM_LOCALES)(
    'keeps core review roles translated in %s settings and agents namespaces',
    (locale) => {
      const settingsReview = readLocaleJson(locale, 'settings/review.json');
      const scenesAgents = readLocaleJson(locale, 'scenes/agents.json');

      for (const role of FALLBACK_REVIEW_TEAM_DEFINITION.coreRoles) {
        expectNonEmptyLocaleString(settingsReview, `members.${role.key}.name`);
        expectNonEmptyLocaleString(settingsReview, `members.${role.key}.role`);

        expectNonEmptyLocaleString(scenesAgents, `reviewTeams.members.${role.key}.funName`);
        expectNonEmptyLocaleString(scenesAgents, `reviewTeams.members.${role.key}.role`);
        expectNonEmptyLocaleString(scenesAgents, `reviewTeams.members.${role.key}.description`);

        role.responsibilities.forEach((_, index) => {
          expectNonEmptyLocaleString(
            scenesAgents,
            `reviewTeams.members.${role.key}.responsibilities.${index}`,
          );
        });
      }
    },
  );

  it.each(REVIEW_TEAM_LOCALES)(
    'keeps Deep Review strategy recommendation UI translated in %s flow chat namespace',
    (locale) => {
      const flowChat = readLocaleJson(locale, 'flow-chat.json');

      for (const path of REVIEW_TEAM_FLOW_CHAT_KEYS) {
        expectNonEmptyLocaleString(flowChat, path);
      }
    },
  );
});
