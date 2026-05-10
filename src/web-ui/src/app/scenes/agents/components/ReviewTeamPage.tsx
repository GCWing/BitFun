import React, { Component, useCallback, useEffect, useMemo, useState } from 'react';
import {
  ArrowLeft,
  BadgeCheck,
  Blocks,
  Bot,
  Gauge,
  GitBranch,
  Layout,
  Lock,
  Settings,
  Shield,
  Users,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, ConfigPageLoading } from '@/component-library';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageSection,
} from '@/infrastructure/config/components/common';
import type { AIModelConfig } from '@/infrastructure/config/types';
import { getModelDisplayName } from '@/infrastructure/config/services/modelConfigs';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';

import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useAgentsStore } from '../agentsStore';
import {
  DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY,
  DEFAULT_REVIEW_TEAM_EXECUTION_POLICY,
  DEFAULT_REVIEW_TEAM_MODEL,
  FALLBACK_REVIEW_TEAM_DEFINITION,
  loadDefaultReviewTeam,
  REVIEW_STRATEGY_DEFINITIONS,
  type ReviewStrategyLevel,
  type ReviewTeam,
  type ReviewTeamMember,
} from '@/shared/services/reviewTeamService';
import '../AgentsView.scss';
import './AgentTeamCard.scss';
import './ReviewTeamPage.scss';

const rtLog = createLogger('ReviewTeamPage');
const DEFAULT_MEMBER_ACCENT = '#64748b';

function getMemberIcon(member: ReviewTeamMember) {
  switch (member.definitionKey) {
    case 'businessLogic':
      return GitBranch;
    case 'performance':
      return Gauge;
    case 'security':
      return Shield;
    case 'architecture':
      return Blocks;
    case 'frontend':
      return Layout;
    case 'judge':
      return BadgeCheck;
    default:
      return Bot;
  }
}

function getMemberResponsibilities(member: ReviewTeamMember): string[] {
  return Array.isArray(member.responsibilities) ? member.responsibilities : [];
}

interface ReviewTeamErrorBoundaryProps {
  children: React.ReactNode;
}

interface ReviewTeamErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class ReviewTeamErrorBoundary extends Component<ReviewTeamErrorBoundaryProps, ReviewTeamErrorBoundaryState> {
  constructor(props: ReviewTeamErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ReviewTeamErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    rtLog.error('ReviewTeamPage render error', {
      message: error.message,
      stack: error.stack,
      componentStack: errorInfo.componentStack,
    });
  }

  render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    return (
      <ConfigPageLayout className="review-team-page">
        <ConfigPageContent>
          <ConfigPageSection title="Code Review Team" description={this.state.error?.message ?? 'Unknown error'}>
            <pre className="review-team-page__error-detail">
              {import.meta.env.DEV ? this.state.error?.stack : null}
            </pre>
          </ConfigPageSection>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }
}

const ReviewTeamPage: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { t: tModel } = useTranslation('settings/default-model');
  const { openHome } = useAgentsStore();
  const setSettingsTab = useSettingsStore((state) => state.setActiveTab);
  const openScene = useSceneStore((state) => state.openScene);
  const { workspacePath } = useCurrentWorkspace();
  const { error: notifyError } = useNotification();

  const [loading, setLoading] = useState(true);
  const [team, setTeam] = useState<ReviewTeam | null>(null);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [selectedMemberId, setSelectedMemberId] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    rtLog.info('loadData started', { workspacePath });
    try {
      const [loadedTeam, loadedModels] = await Promise.all([
        loadDefaultReviewTeam(workspacePath || undefined),
        configAPI.getConfig('ai.models'),
      ]);

      rtLog.info('loadData succeeded', {
        teamId: loadedTeam.id,
        memberCount: loadedTeam.members.length,
        strategyLevel: loadedTeam.strategyLevel,
      });

      setTeam(loadedTeam);
      setModels(Array.isArray(loadedModels) ? loadedModels as AIModelConfig[] : []);
      setSelectedMemberId((currentId) =>
        currentId && loadedTeam.members.some((member) => member.id === currentId)
          ? currentId
          : loadedTeam.members[0]?.id ?? null,
      );
    } catch (error) {
      rtLog.error('loadData failed', {
        message: error instanceof Error ? error.message : String(error),
        stack: error instanceof Error ? error.stack : undefined,
      });
      setTeam({
        id: 'default-review-team',
        name: 'Code Review Team',
        description: '',
        warning: t('reviewTeams.detail.warning', {
          defaultValue: 'Deep review may take longer and usually consumes more tokens than a standard review.',
        }),
        strategyLevel: 'normal',
        memberStrategyOverrides: {},
        executionPolicy: { ...DEFAULT_REVIEW_TEAM_EXECUTION_POLICY },
        concurrencyPolicy: { ...DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY },
        definition: FALLBACK_REVIEW_TEAM_DEFINITION,
        members: [],
        coreMembers: [],
        extraMembers: [],
      });
      setModels([]);
      notifyError(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }, [notifyError, t, workspacePath]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const selectedMember = useMemo(() => {
    if (!team) return null;
    return team.members.find((member) => member.id === selectedMemberId) ?? team.members[0] ?? null;
  }, [selectedMemberId, team]);

  const getLocalizedMemberName = useCallback((member: ReviewTeamMember): string => {
    if (!member.definitionKey) return member.displayName;
    return t(`reviewTeams.members.${member.definitionKey}.role`, {
      defaultValue: member.roleName,
    });
  }, [t]);

  const getLocalizedMemberDescription = useCallback((member: ReviewTeamMember): string => {
    if (!member.definitionKey) {
      return t('reviewTeams.extraReviewer.description', { defaultValue: member.description });
    }
    return t(`reviewTeams.members.${member.definitionKey}.description`, {
      defaultValue: member.description,
    });
  }, [t]);

  const getLocalizedResponsibilities = useCallback((member: ReviewTeamMember): string[] => {
    const baseKey = member.definitionKey
      ? `reviewTeams.members.${member.definitionKey}.responsibilities`
      : 'reviewTeams.extraReviewer.responsibilities';

    return getMemberResponsibilities(member).map((item, index) =>
      t(`${baseKey}.${index}`, { defaultValue: item }),
    );
  }, [t]);

  const getStrategyLabel = useCallback((level: ReviewStrategyLevel): string => {
    return t(`reviewTeams.strategy.${level}.label`, {
      defaultValue: REVIEW_STRATEGY_DEFINITIONS[level].label,
    });
  }, [t]);

  const formatPolicySeconds = useCallback((seconds: number): string => {
    if (seconds <= 0) {
      return t('reviewTeams.detail.noTimeout', { defaultValue: 'No cap' });
    }
    return t('reviewTeams.detail.secondsValue', {
      seconds,
      defaultValue: `${seconds}s`,
    });
  }, [t]);

  const formatSplitThreshold = useCallback((count: number): string => {
    if (count <= 0) {
      return t('reviewTeams.detail.splitDisabled', { defaultValue: 'No split' });
    }
    return t('reviewTeams.detail.fileCountValue', {
      count,
      defaultValue: `${count} files`,
    });
  }, [t]);

  const formatModelLabel = useCallback((modelId: string): string => {
    if (!modelId || modelId === DEFAULT_REVIEW_TEAM_MODEL) {
      return tModel('selection.fast', { defaultValue: 'Fast' });
    }
    if (modelId === 'primary') {
      return tModel('selection.primary', { defaultValue: 'Primary' });
    }

    const match = models.find((model) => model.id === modelId);
    return match ? getModelDisplayName(match) : modelId;
  }, [models, tModel]);

  const reviewTeamCoreMemberNames = useMemo(
    () => (team?.coreMembers ?? []).map((member) =>
      member.definitionKey
        ? t(`reviewTeams.members.${member.definitionKey}.role`, {
          defaultValue: member.roleName,
        })
        : member.displayName,
    ),
    [team?.coreMembers, t],
  );

  const reviewTeamMembersLabel = useMemo(
    () =>
      team
        ? t('reviewTeams.default.members', {
          count: team.members.length,
          defaultValue: `${team.members.length} members`,
        })
        : '',
    [team, t],
  );

  const openReviewSettings = useCallback(() => {
    setSettingsTab('review');
    openScene('settings');
  }, [openScene, setSettingsTab]);

  if (loading || !team) {
    return (
      <ConfigPageLayout className="review-team-page">
        <ConfigPageLoading text={t('reviewTeams.detail.loading', { defaultValue: 'Loading code review team...' })} />
      </ConfigPageLayout>
    );
  }


  const policy = team.executionPolicy;
  const strategyLabel = getStrategyLabel(team.strategyLevel);
  const reviewerTimeoutLabel = formatPolicySeconds(policy.reviewerTimeoutSeconds);
  const judgeTimeoutLabel = formatPolicySeconds(policy.judgeTimeoutSeconds);
  const splitThresholdLabel = formatSplitThreshold(policy.reviewerFileSplitThreshold);
  const sameRoleInstancesLabel = t('reviewTeams.detail.instancesValue', {
    count: policy.maxSameRoleInstances,
    defaultValue: `${policy.maxSameRoleInstances} max`,
  });
  const policySummaryDescription = t('reviewTeams.detail.policySummaryDescription', {
    strategy: strategyLabel,
    reviewerTimeout: reviewerTimeoutLabel,
    judgeTimeout: judgeTimeoutLabel,
    splitThreshold: splitThresholdLabel,
    maxSameRoleInstances: sameRoleInstancesLabel,
    defaultValue:
      `${strategyLabel} review uses ${reviewerTimeoutLabel} reviewer timeouts, ` +
      `${judgeTimeoutLabel} judge timeouts, splits at ${splitThresholdLabel}, ` +
      `and allows ${sameRoleInstancesLabel} per reviewer role.`,
  });

  return (
    <ConfigPageLayout className="review-team-page">
      <ConfigPageHeader
        title={t('reviewTeams.detail.title', { defaultValue: 'Code Review Team' })}
        subtitle={t('reviewTeams.detail.subtitle', {
          defaultValue:
            'Inspect the Code Review Team used by Deep Review and /DeepReview. Strategy and reviewer settings live in Settings.',
        })}
        extra={(
          <div className="review-team-page__header-actions">
            <Button variant="secondary" size="small" onClick={openReviewSettings}>
              <Settings size={14} style={{ marginRight: 6 }} />
              {t('reviewTeams.detail.openSettings', { defaultValue: 'Review settings' })}
            </Button>
            <Button variant="secondary" size="small" onClick={openHome}>
              <ArrowLeft size={14} style={{ marginRight: 6 }} />
              {t('reviewTeams.detail.back', { defaultValue: 'Back to Agents' })}
            </Button>
          </div>
        )}
      />

      <ConfigPageContent>
        <ConfigPageSection
          className="review-team-page__section--no-body-frame"
          title={t('reviewTeams.detail.summaryTitle', { defaultValue: 'Team Overview' })}
          description={t('reviewTeams.detail.summaryDescription', {
            defaultValue: 'The code review team launches reviewers in parallel and finishes with a quality-gate pass.',
          })}
        >
          <div className="review-team-page__agent-team-metrics-wrap">
            <div
              className="agent-team-card__metrics"
              aria-label={reviewTeamCoreMemberNames.join(', ')}
            >
              <Badge variant="neutral">
                <Users size={10} />
                {reviewTeamMembersLabel}
              </Badge>
              <Badge variant="accent">
                <GitBranch size={10} />
                {t('reviewTeams.detail.localOnly', { defaultValue: 'Code review' })}
              </Badge>
              <Badge variant="purple">
                <BadgeCheck size={10} />
                {t('reviewTeams.detail.qualityGate', { defaultValue: 'Quality gate' })}
              </Badge>
            </div>
          </div>
          <div className="review-team-page__summary-grid">
            <div className="review-team-page__summary-card review-team-page__summary-card--primary">
              <div className="review-team-page__summary-card-head">
                <span className="review-team-page__summary-card-icon" aria-hidden>
                  <GitBranch size={14} strokeWidth={1.8} />
                </span>
                <span className="review-team-page__summary-kicker">
                  {t('reviewTeams.detail.localOnly', { defaultValue: 'Code review' })}
                </span>
              </div>
              <p className="review-team-page__summary-value">
                {t('reviewTeams.detail.localOnlyDescription', {
                  defaultValue: 'Reviewers run as BitFun subagents and report through the same review workflow.',
                })}
              </p>
            </div>
            <div className="review-team-page__summary-card">
              <div className="review-team-page__summary-card-head">
                <span className="review-team-page__summary-card-icon" aria-hidden>
                  <Users size={14} strokeWidth={1.8} />
                </span>
                <span className="review-team-page__summary-kicker">
                  {t('reviewTeams.detail.parallelLabel', { defaultValue: 'Parallel reviewers' })}
                </span>
              </div>
              <p className="review-team-page__summary-value">
                {t('reviewTeams.detail.parallelDescription', {
                  defaultValue: 'Business logic, performance, security, and extra reviewers run concurrently before the judge verifies them.',
                })}
              </p>
            </div>
            <div className="review-team-page__summary-card">
              <div className="review-team-page__summary-card-head">
                <span className="review-team-page__summary-card-icon" aria-hidden>
                  <BadgeCheck size={14} strokeWidth={1.8} />
                </span>
                <span className="review-team-page__summary-kicker">
                  {t('reviewTeams.detail.qualityGate', { defaultValue: 'Quality gate' })}
                </span>
              </div>
              <p className="review-team-page__summary-value">
                {t('reviewTeams.detail.warning', { defaultValue: team.warning })}
              </p>
            </div>
          </div>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('reviewTeams.detail.policySummaryTitle', { defaultValue: 'Current Policy' })}
          description={t('reviewTeams.detail.policySummaryIntro', {
            defaultValue: 'This live snapshot comes from Review settings and updates when the team policy changes.',
          })}
          extra={(
            <Button variant="secondary" size="small" onClick={openReviewSettings}>
              <Settings size={14} style={{ marginRight: 6 }} />
              {t('reviewTeams.detail.openSettings', { defaultValue: 'Review settings' })}
            </Button>
          )}
        >
          <button
            type="button"
            className="review-team-page__policy-panel"
            onClick={openReviewSettings}
            aria-label={t('reviewTeams.detail.policySummaryAction', {
              defaultValue: 'Open Review settings to edit the current policy',
            })}
          >
            <div className="review-team-page__policy-copy">
              <span className="review-team-page__policy-eyebrow">
                {t('reviewTeams.detail.policySummaryEyebrow', { defaultValue: 'Configured behavior' })}
              </span>
              <p className="review-team-page__policy-description">{policySummaryDescription}</p>
            </div>

            <div
              className="review-team-page__policy-metrics"
              aria-label={t('reviewTeams.detail.policyMetricsLabel', {
                defaultValue: 'Current policy values',
              })}
            >
              <span className="review-team-page__policy-metric">
                <span>{t('reviewTeams.detail.policyStrategyLabel', { defaultValue: 'Strategy' })}</span>
                <strong>{strategyLabel}</strong>
              </span>
              <span className="review-team-page__policy-metric">
                <span>{t('reviewTeams.detail.reviewerTimeout', { defaultValue: 'Reviewer timeout' })}</span>
                <strong>{reviewerTimeoutLabel}</strong>
              </span>
              <span className="review-team-page__policy-metric">
                <span>{t('reviewTeams.detail.judgeTimeout', { defaultValue: 'Judge timeout' })}</span>
                <strong>{judgeTimeoutLabel}</strong>
              </span>
              <span className="review-team-page__policy-metric">
                <span>{t('reviewTeams.detail.fileSplitThreshold', { defaultValue: 'File split threshold' })}</span>
                <strong>{splitThresholdLabel}</strong>
              </span>
              <span className="review-team-page__policy-metric">
                <span>{t('reviewTeams.detail.maxSameRoleInstances', { defaultValue: 'Max same-role instances' })}</span>
                <strong>{sameRoleInstancesLabel}</strong>
              </span>
            </div>


          </button>
        </ConfigPageSection>

        <ConfigPageSection
          className="review-team-page__section--no-body-frame"
          title={t('reviewTeams.detail.membersTitle', { defaultValue: 'Team Members' })}
          description={t('reviewTeams.detail.membersDescription', {
            defaultValue: 'Click a member to inspect its role and responsibilities. Core roles always stay in the team.',
          })}
          extra={(
            <div className="review-team-page__section-badges">
              <Badge variant="info">
                {t('reviewTeams.detail.lockedCount', {
                  count: team.coreMembers.length,
                  defaultValue: `${team.coreMembers.length} locked roles`,
                })}
              </Badge>
              <Badge variant="neutral">
                {t('reviewTeams.detail.extraCount', {
                  count: team.extraMembers.length,
                  defaultValue: `${team.extraMembers.length} extra Sub-Agents`,
                })}
              </Badge>
            </div>
          )}
        >
          <div className="review-team-page__member-layout">
            <div className="review-team-page__member-list">
              {team.members.map((member) => {
                const MemberIcon = getMemberIcon(member);
                const isSelected = selectedMember?.id === member.id;

                return (
                  <button
                    key={member.id}
                    type="button"
                    className={`review-team-page__member-list-item${isSelected ? ' is-selected' : ''}`}
                    style={{ '--member-accent': member.accentColor || DEFAULT_MEMBER_ACCENT } as React.CSSProperties}
                    onClick={() => setSelectedMemberId(isSelected ? null : member.id)}
                  >
                    <div className="review-team-page__member-list-icon">
                      <MemberIcon size={16} strokeWidth={1.9} />
                    </div>
                    <div className="review-team-page__member-list-body">
                      <span className="review-team-page__member-list-name">
                        {getLocalizedMemberName(member)}
                      </span>
                      <span className="review-team-page__member-list-meta">
                        {formatModelLabel(member.model)}
                      </span>
                    </div>
                    <div className="review-team-page__member-list-badges">
                      {member.locked ? (
                        <Badge variant="neutral">
                          <Lock size={10} />
                          {t('reviewTeams.detail.memberTypes.locked', { defaultValue: 'Locked' })}
                        </Badge>
                      ) : (
                        <Badge variant="info">
                          {t('reviewTeams.detail.memberTypes.extra', { defaultValue: 'Extra' })}
                        </Badge>
                      )}
                    </div>
                  </button>
                );
              })}
            </div>

            {selectedMember ? (
              <div
                className="review-team-page__member-detail-panel"
                style={{ '--member-accent': selectedMember.accentColor || DEFAULT_MEMBER_ACCENT } as React.CSSProperties}
              >
                <div className="review-team-page__detail-hero">
                  <div className="review-team-page__detail-icon">
                    {(() => {
                      const DetailIcon = getMemberIcon(selectedMember);
                      return <DetailIcon size={18} strokeWidth={1.9} />;
                    })()}
                  </div>
                  <div className="review-team-page__detail-copy">
                    <div className="review-team-page__detail-title-row">
                      <div>
                        <h3 className="review-team-page__detail-name">
                          {getLocalizedMemberName(selectedMember)}
                        </h3>
                        <p className="review-team-page__detail-role">
                          {selectedMember.subagentId}
                        </p>
                      </div>
                      <div className="review-team-page__detail-badges">
                        <Badge variant="accent">{formatModelLabel(selectedMember.model)}</Badge>
                        <Badge variant={selectedMember.strategySource === 'member' ? 'info' : 'neutral'}>
                          {getStrategyLabel(selectedMember.strategyLevel)}
                        </Badge>
                        {selectedMember.locked ? (
                          <Badge variant="neutral">
                            {t('reviewTeams.detail.memberTypes.core', { defaultValue: 'Core role' })}
                          </Badge>
                        ) : null}
                      </div>
                    </div>
                  </div>
                </div>
                <p className="review-team-page__detail-description">
                  {getLocalizedMemberDescription(selectedMember)}
                </p>

                <div className="review-team-page__responsibilities">
                  <span className="review-team-page__block-label">
                    {t('reviewTeams.detail.responsibilities', { defaultValue: 'Responsibilities' })}
                  </span>
                  <ul className="review-team-page__responsibility-list">
                    {getLocalizedResponsibilities(selectedMember).map((item, index) => (
                      <li key={`${selectedMember.id}-${index}`} className="review-team-page__responsibility-item">
                        {item}
                      </li>
                    ))}
                  </ul>
                </div>
              </div>
            ) : null}
          </div>
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export { ReviewTeamErrorBoundary };
export default ReviewTeamPage;
