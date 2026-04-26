import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, ConfigPageLoading, NumberInput, Select } from '@/component-library';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import type { AIModelConfig } from '../types';
import { getModelDisplayName } from '../services/modelConfigs';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { SubagentAPI, type SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useNotification } from '@/shared/notification-system';
import {
  addDefaultReviewTeamMember,
  canUseSubagentAsReviewTeamMember,
  DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL,
  DEFAULT_REVIEW_TEAM_MODEL,
  loadDefaultReviewTeam,
  removeDefaultReviewTeamMember,
  REVIEW_STRATEGY_DEFINITIONS,
  REVIEW_STRATEGY_LEVELS,
  saveDefaultReviewTeamExecutionPolicy,
  saveDefaultReviewTeamMemberStrategyOverride,
  saveDefaultReviewTeamStrategyLevel,
  type ReviewMemberStrategyLevel,
  type ReviewStrategyLevel,
  type ReviewTeam,
  type ReviewTeamExecutionPolicy,
  type ReviewTeamMember,
} from '@/shared/services/reviewTeamService';
import './ReviewConfig.scss';

const MEMBER_STRATEGY_OPTIONS: ReviewMemberStrategyLevel[] = [
  DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL,
  ...REVIEW_STRATEGY_LEVELS,
];

function updateMemberStrategy(
  member: ReviewTeamMember,
  strategyOverride: ReviewMemberStrategyLevel,
  teamStrategyLevel: ReviewStrategyLevel,
): ReviewTeamMember {
  const inheritsTeamStrategy = strategyOverride === DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL;
  return {
    ...member,
    strategyOverride,
    strategyLevel: inheritsTeamStrategy ? teamStrategyLevel : strategyOverride,
    strategySource: inheritsTeamStrategy ? 'team' : 'member',
  };
}

function updateTeamStrategy(team: ReviewTeam, strategyLevel: ReviewStrategyLevel): ReviewTeam {
  const memberStrategyOverrides: Record<string, ReviewStrategyLevel> = {};
  const members = team.members.map((member) => {
    if (member.strategyOverride === DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL) {
      return updateMemberStrategy(member, DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL, strategyLevel);
    }
    memberStrategyOverrides[member.subagentId] = member.strategyOverride;
    return member;
  });

  return {
    ...team,
    strategyLevel,
    memberStrategyOverrides,
    members,
    coreMembers: members.filter((member) => member.source === 'core'),
    extraMembers: members.filter((member) => member.source === 'extra'),
  };
}

function updateTeamMemberStrategy(
  team: ReviewTeam,
  memberId: string,
  strategyLevel: ReviewMemberStrategyLevel,
): ReviewTeam {
  const members = team.members.map((member) =>
    member.id === memberId
      ? updateMemberStrategy(member, strategyLevel, team.strategyLevel)
      : member,
  );
  const memberStrategyOverrides: Record<string, ReviewStrategyLevel> = {};
  members.forEach((member) => {
    if (member.strategyOverride !== DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL) {
      memberStrategyOverrides[member.subagentId] = member.strategyOverride;
    }
  });

  return {
    ...team,
    memberStrategyOverrides,
    members,
    coreMembers: members.filter((member) => member.source === 'core'),
    extraMembers: members.filter((member) => member.source === 'extra'),
  };
}

const ReviewConfig: React.FC = () => {
  const { t } = useTranslation('settings/review');
  const { t: tModel } = useTranslation('settings/default-model');
  const { workspacePath } = useCurrentWorkspace();
  const { error: notifyError, success: notifySuccess } = useNotification();

  const [loading, setLoading] = useState(true);
  const [team, setTeam] = useState<ReviewTeam | null>(null);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [subagents, setSubagents] = useState<SubagentInfo[]>([]);
  const [candidateId, setCandidateId] = useState('');
  const [savingPolicyKey, setSavingPolicyKey] = useState<keyof ReviewTeamExecutionPolicy | null>(null);
  const [savingMemberId, setSavingMemberId] = useState<string | null>(null);
  const [savingStrategyTarget, setSavingStrategyTarget] = useState<string | null>(null);
  const [addingMember, setAddingMember] = useState(false);
  const [removingMemberId, setRemovingMemberId] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const [loadedTeam, loadedModels, loadedSubagents] = await Promise.all([
        loadDefaultReviewTeam(workspacePath || undefined),
        configAPI.getConfig('ai.models'),
        SubagentAPI.listSubagents({ workspacePath: workspacePath || undefined }),
      ]);
      setTeam(loadedTeam);
      setModels(Array.isArray(loadedModels) ? loadedModels as AIModelConfig[] : []);
      setSubagents(loadedSubagents);
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [notifyError, t, workspacePath]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const getMemberName = useCallback((member: ReviewTeamMember) => {
    if (!member.definitionKey) return member.displayName;
    return t(`members.${member.definitionKey}.name`, { defaultValue: member.roleName });
  }, [t]);

  const getMemberRole = useCallback((member: ReviewTeamMember) => {
    if (!member.definitionKey) return t('extra.role', { defaultValue: member.roleName });
    return t(`members.${member.definitionKey}.role`, { defaultValue: member.roleName });
  }, [t]);

  const getStrategyLabel = useCallback((level: ReviewStrategyLevel) => (
    t(`strategy.${level}.label`, { defaultValue: REVIEW_STRATEGY_DEFINITIONS[level].label })
  ), [t]);

  const getStrategySummary = useCallback((level: ReviewStrategyLevel) => (
    t(`strategy.${level}.summary`, { defaultValue: REVIEW_STRATEGY_DEFINITIONS[level].summary })
  ), [t]);

  const getMemberStrategyLabel = useCallback((level: ReviewMemberStrategyLevel) => {
    if (level === DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL) {
      return t('strategy.inherit', {
        level: team ? getStrategyLabel(team.strategyLevel) : '',
        defaultValue: team ? `Inherit team (${getStrategyLabel(team.strategyLevel)})` : 'Inherit team',
      });
    }
    return getStrategyLabel(level);
  }, [getStrategyLabel, t, team]);

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

  const extraCandidates = useMemo(() => {
    if (!team) return [];
    const existingIds = new Set(team.members.map((member) => member.subagentId));

    return subagents
      .filter((subagent) => !existingIds.has(subagent.id))
      .filter(canUseSubagentAsReviewTeamMember)
      .sort((left, right) => left.name.localeCompare(right.name) || left.id.localeCompare(right.id));
  }, [subagents, team]);

  const modelOptions = useMemo(() => [
    { value: DEFAULT_REVIEW_TEAM_MODEL, label: tModel('selection.fast', { defaultValue: 'Fast' }) },
    { value: 'primary', label: tModel('selection.primary', { defaultValue: 'Primary' }) },
    ...models
      .filter((model) => Boolean(model.id))
      .map((model) => ({
        value: model.id as string,
        label: getModelDisplayName(model),
      })),
  ], [models, tModel]);

  useEffect(() => {
    setCandidateId((currentId) =>
      currentId && extraCandidates.some((candidate) => candidate.id === currentId)
        ? currentId
        : extraCandidates[0]?.id ?? '',
    );
  }, [extraCandidates]);

  const handleTeamStrategyChange = useCallback(async (strategyLevel: ReviewStrategyLevel) => {
    if (!team || team.strategyLevel === strategyLevel) return;

    setSavingStrategyTarget('team');
    setTeam(updateTeamStrategy(team, strategyLevel));
    try {
      await saveDefaultReviewTeamStrategyLevel(strategyLevel);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      await loadData();
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingStrategyTarget(null);
    }
  }, [loadData, notifyError, notifySuccess, t, team]);

  const handleMemberStrategyChange = useCallback(async (
    member: ReviewTeamMember,
    strategyLevel: ReviewMemberStrategyLevel,
  ) => {
    if (!team || member.strategyOverride === strategyLevel) return;

    const target = `member:${member.id}`;
    setSavingStrategyTarget(target);
    setTeam(updateTeamMemberStrategy(team, member.id, strategyLevel));
    try {
      await saveDefaultReviewTeamMemberStrategyOverride(member.subagentId, strategyLevel);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      await loadData();
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingStrategyTarget(null);
    }
  }, [loadData, notifyError, notifySuccess, t, team]);

  const handleExecutionPolicyChange = useCallback(async (
    key: keyof ReviewTeamExecutionPolicy,
    value: ReviewTeamExecutionPolicy[keyof ReviewTeamExecutionPolicy],
  ) => {
    if (!team) return;

    const nextPolicy = { ...team.executionPolicy, [key]: value };
    setSavingPolicyKey(key);
    setTeam({ ...team, executionPolicy: nextPolicy });
    try {
      await saveDefaultReviewTeamExecutionPolicy(nextPolicy);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      await loadData();
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingPolicyKey(null);
    }
  }, [loadData, notifyError, notifySuccess, t, team]);

  const handleModelChange = useCallback(async (member: ReviewTeamMember, modelId: string) => {
    setSavingMemberId(member.id);
    try {
      await SubagentAPI.updateSubagentConfig({
        subagentId: member.subagentId,
        enabled: true,
        model: modelId,
        workspacePath: workspacePath || undefined,
      });
      await loadData();
      notifySuccess(t('messages.saved'));
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingMemberId(null);
    }
  }, [loadData, notifyError, notifySuccess, t, workspacePath]);

  const handleAddMember = useCallback(async () => {
    if (!candidateId) return;
    setAddingMember(true);
    try {
      await addDefaultReviewTeamMember(candidateId);
      await SubagentAPI.updateSubagentConfig({
        subagentId: candidateId,
        enabled: true,
        workspacePath: workspacePath || undefined,
      });
      await loadData();
      notifySuccess(t('messages.memberAdded'));
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setAddingMember(false);
    }
  }, [candidateId, loadData, notifyError, notifySuccess, t, workspacePath]);

  const handleRemoveMember = useCallback(async (member: ReviewTeamMember) => {
    if (member.locked) return;

    setRemovingMemberId(member.id);
    try {
      await removeDefaultReviewTeamMember(member.subagentId);
      await loadData();
      notifySuccess(t('messages.memberRemoved'));
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setRemovingMemberId(null);
    }
  }, [loadData, notifyError, notifySuccess, t]);

  if (loading || !team) {
    return (
      <ConfigPageLayout>
        <ConfigPageLoading text={t('loading')} />
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="review-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent>
        <ConfigPageSection
          title={t('overview.title')}
          description={t('overview.description')}
          titleSuffix={<Badge variant="info">{t('overview.badge')}</Badge>}
        >
          <div className="review-config__overview-grid">
            <div className="review-config__overview-item">
              <span className="review-config__overview-label">{t('overview.command.title')}</span>
              <p className="review-config__overview-copy">{t('overview.command.description')}</p>
            </div>
            <div className="review-config__overview-item">
              <span className="review-config__overview-label">{t('overview.reviewers.title')}</span>
              <p className="review-config__overview-copy">{t('overview.reviewers.description')}</p>
            </div>
            <div className="review-config__overview-item">
              <span className="review-config__overview-label">{t('overview.qualityGate.title')}</span>
              <p className="review-config__overview-copy">{t('overview.qualityGate.description')}</p>
            </div>
          </div>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('strategy.title')}
          description={t('strategy.description')}
          titleSuffix={<Badge variant="neutral">{getStrategyLabel(team.strategyLevel)}</Badge>}
        >
          <div className="review-config__strategy-options">
            {REVIEW_STRATEGY_LEVELS.map((level) => {
              const isSelected = team.strategyLevel === level;
              return (
                <button
                  key={level}
                  type="button"
                  className={`review-config__strategy-option${isSelected ? ' is-selected' : ''}`}
                  aria-pressed={isSelected}
                  disabled={savingStrategyTarget === 'team'}
                  onClick={() => void handleTeamStrategyChange(level)}
                >
                  <span className="review-config__strategy-title">{getStrategyLabel(level)}</span>
                  <span className="review-config__strategy-summary">{getStrategySummary(level)}</span>
                </button>
              );
            })}
          </div>
        </ConfigPageSection>

        <ConfigPageSection title={t('execution.title')} description={t('execution.description')}>
          <ConfigPageRow label={t('execution.reviewerTimeout')} description={t('execution.reviewerTimeoutDesc')} align="center" balanced>
            <NumberInput
              value={team.executionPolicy.reviewerTimeoutSeconds}
              onChange={(value) => void handleExecutionPolicyChange('reviewerTimeoutSeconds', value)}
              min={0}
              max={3600}
              step={30}
              unit="s"
              size="small"
              disabled={savingPolicyKey === 'reviewerTimeoutSeconds'}
            />
          </ConfigPageRow>

          <ConfigPageRow label={t('execution.judgeTimeout')} description={t('execution.judgeTimeoutDesc')} align="center" balanced>
            <NumberInput
              value={team.executionPolicy.judgeTimeoutSeconds}
              onChange={(value) => void handleExecutionPolicyChange('judgeTimeoutSeconds', value)}
              min={0}
              max={3600}
              step={30}
              unit="s"
              size="small"
              disabled={savingPolicyKey === 'judgeTimeoutSeconds'}
            />
          </ConfigPageRow>

          <ConfigPageRow label={t('execution.fileSplitThreshold')} description={t('execution.fileSplitThresholdDesc')} align="center" balanced>
            <NumberInput
              value={team.executionPolicy.reviewerFileSplitThreshold}
              onChange={(value) => void handleExecutionPolicyChange('reviewerFileSplitThreshold', value)}
              min={0}
              max={9999}
              step={5}
              size="small"
              disabled={savingPolicyKey === 'reviewerFileSplitThreshold'}
            />
          </ConfigPageRow>

          <ConfigPageRow label={t('execution.maxSameRoleInstances')} description={t('execution.maxSameRoleInstancesDesc')} align="center" balanced>
            <NumberInput
              value={team.executionPolicy.maxSameRoleInstances}
              onChange={(value) => void handleExecutionPolicyChange('maxSameRoleInstances', value)}
              min={1}
              max={8}
              step={1}
              size="small"
              disabled={savingPolicyKey === 'maxSameRoleInstances'}
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('members.title')}
          description={t('members.description')}
          titleSuffix={<Badge variant="neutral">{t('members.count', { count: team.members.length })}</Badge>}
        >
          <div className="review-config__member-list">
            {team.members.map((member) => (
              <div key={member.id} className="review-config__member-row">
                <div className="review-config__member-main">
                  <div className="review-config__member-heading">
                    <span className="review-config__member-name">{getMemberName(member)}</span>
                    <Badge variant={member.locked ? 'neutral' : 'info'}>
                      {member.locked ? t('members.locked') : t('members.extra')}
                    </Badge>
                  </div>
                  <p className="review-config__member-role">{getMemberRole(member)}</p>
                </div>

                <div className="review-config__member-controls">
                  <Select
                    value={member.strategyOverride}
                    onChange={(value) => void handleMemberStrategyChange(
                      member,
                      (Array.isArray(value) ? value[0] : value) as ReviewMemberStrategyLevel,
                    )}
                    size="small"
                    disabled={savingStrategyTarget === `member:${member.id}`}
                    options={MEMBER_STRATEGY_OPTIONS.map((level) => ({
                      value: level,
                      label: getMemberStrategyLabel(level),
                    }))}
                  />
                  <Select
                    value={member.model || DEFAULT_REVIEW_TEAM_MODEL}
                    size="small"
                    disabled={savingMemberId === member.id}
                    options={modelOptions}
                    onChange={(value) => void handleModelChange(
                      member,
                      String(Array.isArray(value) ? value[0] || DEFAULT_REVIEW_TEAM_MODEL : value),
                    )}
                  />
                  <span className="review-config__member-model">{formatModelLabel(member.model)}</span>
                  {!member.locked ? (
                    <Button
                      variant="secondary"
                      size="small"
                      disabled={removingMemberId === member.id}
                      onClick={() => void handleRemoveMember(member)}
                    >
                      {t('members.remove')}
                    </Button>
                  ) : null}
                </div>
              </div>
            ))}
          </div>
        </ConfigPageSection>

        <ConfigPageSection title={t('extra.title')} description={t('extra.description')}>
          <ConfigPageRow label={t('extra.candidate')} description={t('extra.hint')} multiline>
            <div className="review-config__add-row">
              <Select
                value={candidateId}
                onChange={(value) => setCandidateId(String(Array.isArray(value) ? value[0] || '' : value))}
                size="small"
                disabled={extraCandidates.length === 0 || addingMember}
                placeholder={t('extra.placeholder')}
                options={extraCandidates.map((candidate) => ({
                  value: candidate.id,
                  label: `${candidate.name} - ${candidate.subagentSource ?? 'user'}`,
                }))}
              />
              <Button
                variant="primary"
                size="small"
                disabled={!candidateId || extraCandidates.length === 0 || addingMember}
                onClick={() => void handleAddMember()}
              >
                {t('extra.add')}
              </Button>
            </div>
            {extraCandidates.length === 0 ? (
              <p className="review-config__empty">{t('extra.empty')}</p>
            ) : null}
          </ConfigPageRow>
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default ReviewConfig;
