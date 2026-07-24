import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { UI_EXCEPTION_ACCENTS } from '@/shared/theme/uiExceptionAccents';
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
  'deepReviewConsent.callLimit',
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
    dynamicConsentMarkers: string[];
    extraReviewRole: string;
    forbiddenConsentPhrases: string[];
    reviewConsentTitle: string;
    reviewBudgetLabel: string;
  }
> = {
  'en-US': {
    conditionalJudgeMarker: 'only when',
    dynamicConsentMarkers: ['may add', 'review budget'],
    extraReviewRole: 'Additional Review Check',
    forbiddenConsentPhrases: ['selected additional independent checks', 'review agent run'],
    reviewConsentTitle: 'Start this review?',
    reviewBudgetLabel: 'Review budget',
  },
  'zh-CN': {
    conditionalJudgeMarker: '只在',
    dynamicConsentMarkers: ['按需', '审核预算'],
    extraReviewRole: '额外审核检查',
    forbiddenConsentPhrases: ['选择了额外的独立检查', '审查代理'],
    reviewConsentTitle: '开始本次审核？',
    reviewBudgetLabel: '审核预算',
  },
  'zh-TW': {
    conditionalJudgeMarker: '只在',
    dynamicConsentMarkers: ['視需要', '審核預算'],
    extraReviewRole: '額外審核檢查',
    forbiddenConsentPhrases: ['選擇了額外的獨立檢查', '審查代理'],
    reviewConsentTitle: '開始本次審核？',
    reviewBudgetLabel: '審核預算',
  },
};

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
        'deepReviewConsent.callLimit',
        'deepReviewConsent.strategySummaries.normal',
        'deepReviewConsent.strategySummaries.deep',
      ].map((path) => String(getPathValue(flowChat, path) ?? '')).join('\n');
      const expectation = REVIEW_COPY_EXPECTATIONS[locale];

      expect(getPathValue(flowChat, 'deepReviewConsent.title')).toBe(
        expectation.reviewConsentTitle,
      );
      expect(getPathValue(flowChat, 'deepReviewConsent.costLabel')).toBe(
        expectation.reviewBudgetLabel,
      );
      for (const marker of expectation.dynamicConsentMarkers) {
        expect(consentCopy).toContain(marker);
      }
      for (const phrase of expectation.forbiddenConsentPhrases) {
        expect(consentCopy).not.toContain(phrase);
      }
    },
  );

  it('keeps review accent semantics limited to active generic roles', () => {
    expect(Object.keys(UI_EXCEPTION_ACCENTS.reviewTeam).sort()).toEqual([
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
      funName: 'Focused Review',
      roleName: 'On-demand Review Check',
    });
    expect(judge).toMatchObject({
      funName: 'Independent Review Check',
      roleName: 'Review Quality Check',
    });
    expect(FALLBACK_REVIEW_TEAM_DEFINITION.description).toBe(
      'One main review that can request focused independent checks when more evidence is needed.',
    );
    expect(EXTRA_MEMBER_DEFAULTS.roleName).toBe('Additional Review Check');

    const userFacingCopy = [
      worker?.description,
      judge?.description,
      EXTRA_MEMBER_DEFAULTS.description,
      ...Object.values(FALLBACK_REVIEW_TEAM_DEFINITION.strategyProfiles)
        .map((profile) => profile.summary),
    ].join('\n');
    expect(userFacingCopy).not.toMatch(/\b(worker|lens|specialist|inspector)\b/i);
    expect(userFacingCopy).not.toMatch(/\bone (optional|justified|narrowly focused)\b/i);
  });
});
