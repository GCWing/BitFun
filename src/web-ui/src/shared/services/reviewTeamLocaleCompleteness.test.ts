import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { APPEARANCE_DOMAIN_TOKENS } from '@/infrastructure/appearance/appearanceDomainTokens';
import { FALLBACK_REVIEW_TEAM_DEFINITION } from './reviewTeamService';
import { EXTRA_MEMBER_DEFAULTS } from './review-team/defaults';

const REVIEW_TEAM_LOCALES = ['en-US', 'zh-CN', 'zh-TW'] as const;
const CHINESE_REVIEW_LOCALES = ['zh-CN', 'zh-TW'] as const;

const PLAIN_CHINESE_REVIEW_COPY_PATHS = {
  flowChat: [
    'deepReviewActionBar.diagnosticsTitle',
    'deepReviewActionBar.resultRecovery.missingSubmitCodeReview',
    'deepReviewActionBar.resultRecovery.invalidSubmitCodeReview',
    'deepReviewActionBar.resultRecovery.wrongReviewMode',
    'deepReviewActionBar.capacityQueue.controlFailed',
    'deepReviewConsent.skippedGroupTitle',
    'toolCards.taskTool.reviewCoverageLabel',
    'toolCards.taskTool.reviewCoverageDescription',
    'toolCards.taskDetailPanel.stopReviewWorkHint',
    'toolCards.codeReview.runManifest.reviewDepth',
    'toolCards.codeReview.runManifest.skippedGroupTitle',
    'toolCards.codeReview.reliabilityStatus.reduced_scope.label',
    'toolCards.codeReview.reliabilityStatus.reduced_scope.detail',
    'toolCards.codeReview.reliabilityStatus.skipped_reviewers.label',
  ],
  scenesAgents: [
    'agentsOverview.form.reviewToolsHint',
    'agentDescriptions.DeepReview',
  ],
} as const;

type Locale = (typeof REVIEW_TEAM_LOCALES)[number];
type JsonObject = Record<string, unknown>;

const REVIEW_TEAM_FLOW_CHAT_KEYS = [
  'deepReviewConsent.runStrategy',
  'deepReviewConsent.skippedSummary',
  'deepReviewConsent.strategyLabels.quick',
  'deepReviewConsent.strategyLabels.normal',
  'deepReviewConsent.strategyLabels.deep',
  'toolCards.taskTool.reviewCoverageLabel',
  'toolCards.taskTool.reviewCoverageDescription',
  'toolCards.codeReview.runManifest.recommendedStrategy',
  'toolCards.codeReview.runManifest.riskRecommendationTitle',
  'toolCards.codeReview.runManifest.reviewDepth',
  'toolCards.codeReview.runManifest.reviewDepthLabels.high_risk_only',
  'toolCards.codeReview.runManifest.reviewDepthLabels.risk_expanded',
  'toolCards.codeReview.runManifest.reviewDepthLabels.full_depth',
  'toolCards.codeReview.runManifest.reducedCoverageSummary',
  'toolCards.codeReview.reliabilityStatus.reduced_scope.label',
  'toolCards.codeReview.reliabilityStatus.reduced_scope.detail',
  'toolCards.codeReview.reliabilityStatus.target_evidence_limited.label',
  'toolCards.codeReview.reliabilityStatus.target_evidence_limited.detail',
] as const;

const REVIEW_COPY_EXPECTATIONS: Record<
  Locale,
  {
    conditionalJudgeMarker: string;
    additionalCheckLabel: string;
    dynamicConsentMarkers: string[];
    extraReviewRole: string;
    forbiddenConsentPhrases: string[];
    forbiddenVisiblePhrases: string[];
    reviewConsentTitle: string;
    reviewDepthLabel: string;
  }
> = {
  'en-US': {
    conditionalJudgeMarker: 'only when',
    additionalCheckLabel: 'Additional check',
    dynamicConsentMarkers: ['additional checks', 'evidence'],
    extraReviewRole: 'User-requested check',
    forbiddenConsentPhrases: ['focused check', 'review agent run'],
    forbiddenVisiblePhrases: ['Focused check', 'Review check', 'focused independent'],
    reviewConsentTitle: 'Start this review?',
    reviewDepthLabel: 'Review depth',
  },
  'zh-CN': {
    conditionalJudgeMarker: '只在',
    additionalCheckLabel: '补充检查',
    dynamicConsentMarkers: ['补充检查', '证据'],
    extraReviewRole: '用户指定的检查',
    forbiddenConsentPhrases: ['专项检查', '审查代理'],
    forbiddenVisiblePhrases: ['专项检查', '专项复核', '审核检查'],
    reviewConsentTitle: '开始本次审核？',
    reviewDepthLabel: '审核深度',
  },
  'zh-TW': {
    conditionalJudgeMarker: '只在',
    additionalCheckLabel: '補充檢查',
    dynamicConsentMarkers: ['補充檢查', '證據'],
    extraReviewRole: '使用者指定的檢查',
    forbiddenConsentPhrases: ['專項檢查', '審查代理'],
    forbiddenVisiblePhrases: ['專項檢查', '專項複核', '審核檢查'],
    reviewConsentTitle: '開始本次審核？',
    reviewDepthLabel: '審核深度',
  },
};

function readLocaleJson(
  locale: Locale,
  namespace: 'flow-chat.json' | 'scenes/agents.json' | 'settings/review-capacity.json',
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
    'keeps core review role details translated in %s agents namespace',
    (locale) => {
      const scenesAgents = readLocaleJson(locale, 'scenes/agents.json');
      const members = getPathValue(scenesAgents, 'reviewTeams.members') as JsonObject;
      const expectedMemberKeys = FALLBACK_REVIEW_TEAM_DEFINITION.coreRoles
        .map((role) => role.key)
        .sort();

      expect(Object.keys(members).sort()).toEqual(expectedMemberKeys);

      for (const role of FALLBACK_REVIEW_TEAM_DEFINITION.coreRoles) {
        expectNonEmptyLocaleString(scenesAgents, `reviewTeams.members.${role.key}.funName`);
        expectNonEmptyLocaleString(scenesAgents, `reviewTeams.members.${role.key}.role`);
        expectNonEmptyLocaleString(scenesAgents, `reviewTeams.members.${role.key}.description`);

        role.responsibilities.forEach((_, index) => {
          expectNonEmptyLocaleString(
            scenesAgents,
            `reviewTeams.members.${role.key}.responsibilities.${index}`,
          );
        });

        const translatedResponsibilities = getPathValue(
          scenesAgents,
          `reviewTeams.members.${role.key}.responsibilities`,
        );
        expect(translatedResponsibilities).toHaveLength(role.responsibilities.length);
      }

      expect(
        getPathValue(scenesAgents, 'reviewTeams.members.judge.description'),
      ).toContain(REVIEW_COPY_EXPECTATIONS[locale].conditionalJudgeMarker);
      expect(getPathValue(scenesAgents, 'reviewTeams.extraReviewer.role')).toBe(
        REVIEW_COPY_EXPECTATIONS[locale].extraReviewRole,
      );
      expect(getPathValue(scenesAgents, 'reviewTeams.members.worker.funName')).toBe(
        REVIEW_COPY_EXPECTATIONS[locale].additionalCheckLabel,
      );
      expectNonEmptyLocaleString(
        scenesAgents,
        'reviewTeams.extraReviewer.description',
      );
      EXTRA_MEMBER_DEFAULTS.responsibilities.forEach((_, index) => {
        expectNonEmptyLocaleString(
          scenesAgents,
          `reviewTeams.extraReviewer.responsibilities.${index}`,
        );
      });
      expect(
        getPathValue(scenesAgents, 'reviewTeams.extraReviewer.responsibilities'),
      ).toHaveLength(EXTRA_MEMBER_DEFAULTS.responsibilities.length);
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

  it.each(REVIEW_TEAM_LOCALES)(
    'describes optional dynamic review work without implying a fixed plan in %s',
    (locale) => {
      const flowChat = readLocaleJson(locale, 'flow-chat.json');
      const consentCopy = [
        'deepReviewConsent.body',
        'deepReviewConsent.cost',
        'deepReviewConsent.strategySummaries.normal',
        'deepReviewConsent.strategySummaries.deep',
      ].map((path) => String(getPathValue(flowChat, path) ?? '')).join('\n');
      const expectation = REVIEW_COPY_EXPECTATIONS[locale];

      expect(getPathValue(flowChat, 'deepReviewConsent.title')).toBe(
        expectation.reviewConsentTitle,
      );
      expect(getPathValue(flowChat, 'deepReviewConsent.costLabel')).toBe(
        expectation.reviewDepthLabel,
      );
      expect(getPathValue(flowChat, 'toolCards.taskTool.reviewCoverageLabel')).toBe(
        expectation.additionalCheckLabel,
      );
      expect(getPathValue(flowChat, 'toolCards.codeReview.coverageSources.focusedCheck')).toBe(
        expectation.additionalCheckLabel,
      );
      for (const marker of expectation.dynamicConsentMarkers) {
        expect(consentCopy).toContain(marker);
      }
      for (const phrase of expectation.forbiddenConsentPhrases) {
        expect(consentCopy).not.toContain(phrase);
      }
    },
  );

  it.each(REVIEW_TEAM_LOCALES)(
    'keeps additional-check terminology consistent across user-visible review copy in %s',
    (locale) => {
      const flowChat = readLocaleJson(locale, 'flow-chat.json');
      const scenesAgents = readLocaleJson(locale, 'scenes/agents.json');
      const settings = readLocaleJson(locale, 'settings/review-capacity.json');
      const visibleCopy = [
        'deepReviewConsent.body',
        'deepReviewConsent.cost',
        'deepReviewConsent.readonly',
        'deepReviewConsent.strategySummaries.normal',
        'deepReviewConsent.strategySummaries.deep',
        'deepReviewConsent.skippedSummary',
        'deepReviewConsent.skippedGroupTitle',
        'toolCards.taskTool.reviewCoverageLabel',
        'toolCards.taskTool.reviewCoverageDescription',
        'toolCards.taskTool.reviewCheckUnavailable',
        'toolCards.codeReview.runManifest.estimatedCalls',
        'toolCards.codeReview.runManifest.reducedCoverageSummary',
        'toolCards.codeReview.reliabilityStatus.context_pressure.detail',
        'toolCards.codeReview.coverageSources.focusedCheck',
      ].map((path) => String(getPathValue(flowChat, path) ?? ''));
      visibleCopy.push(
        ...[
          'agentsOverview.form.reviewToolsHint',
          'reviewTeams.members.worker.funName',
          'reviewTeams.members.worker.role',
          'reviewTeams.members.worker.description',
          'reviewTeams.extraReviewer.role',
          'reviewTeams.extraReviewer.description',
        ].map((path) => String(getPathValue(scenesAgents, path) ?? '')),
        ...[
          'capacity.description',
          'capacity.maxParallelReviewers.label',
          'capacity.maxParallelReviewers.description',
        ].map((path) => String(getPathValue(settings, path) ?? '')),
      );

      const joined = visibleCopy.join('\n');
      expect(joined).toContain(REVIEW_COPY_EXPECTATIONS[locale].additionalCheckLabel);
      for (const phrase of REVIEW_COPY_EXPECTATIONS[locale].forbiddenVisiblePhrases) {
        expect(joined.toLowerCase()).not.toContain(phrase.toLowerCase());
      }
    },
  );

  it('keeps review accent semantics limited to active generic roles', () => {
    expect(Object.keys(APPEARANCE_DOMAIN_TOKENS.reviewTeam).sort()).toEqual([
      'judge',
      'memberDefault',
      'worker',
    ]);
  });

  it.each(CHINESE_REVIEW_LOCALES)(
    'keeps user-facing review copy free of internal English terms in %s',
    (locale) => {
      const flowChat = readLocaleJson(locale, 'flow-chat.json');
      const scenesAgents = readLocaleJson(locale, 'scenes/agents.json');
      const visibleCopy = [
        ...PLAIN_CHINESE_REVIEW_COPY_PATHS.flowChat.map(
          (path) => String(getPathValue(flowChat, path) ?? ''),
        ),
        ...PLAIN_CHINESE_REVIEW_COPY_PATHS.scenesAgents.map(
          (path) => String(getPathValue(scenesAgents, path) ?? ''),
        ),
      ].join('\n');

      expect(visibleCopy).not.toMatch(/\b(Review|agent|scope)\b/i);
    },
  );

  it('keeps fallback review copy readable and free of implementation role terms', () => {
    const worker = FALLBACK_REVIEW_TEAM_DEFINITION.coreRoles.find(
      (role) => role.key === 'worker',
    );
    const judge = FALLBACK_REVIEW_TEAM_DEFINITION.coreRoles.find(
      (role) => role.key === 'judge',
    );

    expect(worker).toMatchObject({
      funName: 'Additional check',
      roleName: 'On-demand check',
    });
    expect(judge).toMatchObject({
      funName: 'Independent validation',
      roleName: 'Quality check',
    });
    expect(FALLBACK_REVIEW_TEAM_DEFINITION.description).toBe(
      'One review that can add checks when a specific concern needs more evidence.',
    );
    expect(EXTRA_MEMBER_DEFAULTS.roleName).toBe('User-requested check');

    const userFacingCopy = [
      worker?.description,
      judge?.description,
      EXTRA_MEMBER_DEFAULTS.description,
      ...Object.values(FALLBACK_REVIEW_TEAM_DEFINITION.strategyProfiles)
        .map((profile) => profile.summary),
    ].join('\n');
    expect(userFacingCopy).not.toMatch(/\b(worker|lens|specialist|inspector|focused)\b/i);
    expect(userFacingCopy).not.toMatch(/\bone (optional|justified|narrowly focused)\b/i);
  });
});
