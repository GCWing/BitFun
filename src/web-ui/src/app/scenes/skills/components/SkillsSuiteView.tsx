import { Button, Icon, IconButton, ScrollArea, StatusPill } from '@openbitfun/ui';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Package, RotateCcw, ShieldAlert, ShieldCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { confirmDialog } from '@/infrastructure/confirm-dialog';
import { configAPI } from '@/infrastructure/api';
import { useWorkspaceManagerSync } from '@/infrastructure/hooks/useWorkspaceManagerSync';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';
import type { ModeSkillInfo } from '@/infrastructure/config/types';
import { buildSkillCoverageSourceMap } from '@/infrastructure/config/skillSourcePresentation';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { SkillGroupManagerModal } from '../../agents/components/SkillGroupPicker';
import {
  type ResolvedSkillGroupKind,
  builtinSkillGroupLabelKey,
  resolveSkillGroups,
} from '../../agents/components/skillGroups';
import { useUserSkillGroups } from '../../agents/components/useUserSkillGroups';
import type { SuiteModeId } from '../skillsSceneStore';
import { useSkillsSceneStore } from '../skillsSceneStore';

const log = createLogger('SkillsSuiteView');

const SUITE_MODES = [
  { id: 'agentic', labelKey: 'suite.modes.agentic', descKey: 'suite.modeDescriptions.agentic' },
  { id: 'Cowork', labelKey: 'suite.modes.cowork', descKey: 'suite.modeDescriptions.cowork' },
  { id: 'Claw', labelKey: 'shared:agents.claw', descKey: 'suite.modeDescriptions.claw' },
] as const;

type SuiteMode = typeof SUITE_MODES[number];

interface SuiteSkillGroup {
  id: string;
  kind: ResolvedSkillGroupKind;
  label: string;
  skills: ModeSkillInfo[];
  enabledCount: number;
  totalCount: number;
}

type SavingAction = {
  groupKey: string;
  kind: 'save' | 'toggle';
} | null;

function uniqueKeys(keys: Iterable<string>): string[] {
  return [...new Set([...keys].filter(Boolean))];
}

function buildEnabledKeySet(skills: ModeSkillInfo[]): string[] {
  return uniqueKeys(skills.filter((skill) => skill.effectiveEnabled).map((skill) => skill.key));
}

function cloneSet(keys: Iterable<string>): Set<string> {
  return new Set(keys);
}

function buildSuiteSkillGroups(
  skills: ModeSkillInfo[],
  userGroups: Parameters<typeof resolveSkillGroups>[1],
  enabledKeySet: Set<string>,
  t: (key: string) => string,
): SuiteSkillGroup[] {
  return resolveSkillGroups(skills, userGroups, {
    builtin: (groupKey) => {
      const labelKey = builtinSkillGroupLabelKey(groupKey);
      return labelKey ? t(`suite.groups.${labelKey}`) : groupKey;
    },
    other: t('suite.groups.other'),
  }).map((group) => {
    const groupSkills = group.skills as ModeSkillInfo[];
    return {
      id: group.id,
      kind: group.kind,
      label: group.label,
      skills: groupSkills.sort((left, right) => {
        const leftEnabled = enabledKeySet.has(left.key);
        const rightEnabled = enabledKeySet.has(right.key);
        if (leftEnabled && !rightEnabled) return -1;
        if (!leftEnabled && rightEnabled) return 1;
        return left.name.localeCompare(right.name) || left.key.localeCompare(right.key);
      }),
      enabledCount: groupSkills.filter((skill) => enabledKeySet.has(skill.key)).length,
      totalCount: groupSkills.length,
    };
  });
}

function groupSectionLabel(kind: ResolvedSkillGroupKind, t: (key: string) => string): string {
  switch (kind) {
    case 'user':
      return t('suite.sections.myGroups');
    case 'builtin':
      return t('suite.sections.builtin');
    default:
      return t('suite.sections.otherSkills');
  }
}

function buildSkillTitle(
  skill: ModeSkillInfo,
  enabled: boolean,
  shadowed: boolean,
  dirty: boolean,
  coverageSource: string,
  t: (key: string, options?: { source: string }) => string,
): string {
  return [
    skill.description || skill.name,
    !skill.globallyEnabled
      ? t('suite.skillState.globalDisabled')
      : dirty
      ? t('suite.skillState.pending')
      : enabled
        ? t('suite.skillState.enabled')
        : t('suite.skillState.disabled'),
    shadowed ? t('suite.skillState.coveredDetail', { source: coverageSource }) : null,
  ].filter(Boolean).join('\n');
}

function buildGroupKeySet(group: SuiteSkillGroup): Set<string> {
  return new Set(group.skills.map((skill) => skill.key));
}

function isSameKeySet(leftKeys: string[], rightKeys: string[]): boolean {
  if (leftKeys.length !== rightKeys.length) {
    return false;
  }

  const rightKeySet = new Set(rightKeys);
  return leftKeys.every((key) => rightKeySet.has(key));
}

const SkillsSuiteView: React.FC = () => {
  const { t } = useTranslation('scenes/skills');
  const notification = useNotification();
  const { workspacePath } = useWorkspaceManagerSync();
  const suiteModeId = useSkillsSceneStore((state) => state.suiteModeId);
  const setSuiteModeId = useSkillsSceneStore((state) => state.setSuiteModeId);
  const {
    groups: userSkillGroups,
    saveGroups: saveUserSkillGroups,
  } = useUserSkillGroups();

  const [modeSkills, setModeSkills] = useState<ModeSkillInfo[]>([]);
  const [committedEnabledKeys, setCommittedEnabledKeys] = useState<string[]>([]);
  const [draftEnabledKeys, setDraftEnabledKeys] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [savingAction, setSavingAction] = useState<SavingAction>(null);
  const [resettingModeId, setResettingModeId] = useState<SuiteModeId | null>(null);
  const [isGroupManagerOpen, setIsGroupManagerOpen] = useState(false);
  const loadRequestIdRef = useRef(0);

  const currentMode = useMemo(
    () => SUITE_MODES.find((mode) => mode.id === suiteModeId) ?? SUITE_MODES[0],
    [suiteModeId],
  );

  const committedEnabledKeySet = useMemo(
    () => cloneSet(committedEnabledKeys),
    [committedEnabledKeys],
  );
  const draftEnabledKeySet = useMemo(
    () => cloneSet(draftEnabledKeys),
    [draftEnabledKeys],
  );
  const coverageSourceBySkillKey = useMemo(
    () => buildSkillCoverageSourceMap(modeSkills, t('list.item.unknownSource')),
    [modeSkills, t],
  );

  const suiteGroups = useMemo(
    () => buildSuiteSkillGroups(modeSkills, userSkillGroups, draftEnabledKeySet, t),
    [modeSkills, userSkillGroups, draftEnabledKeySet, t],
  );
  const suiteSections = useMemo(() => {
    const sections = new Map<string, SuiteSkillGroup[]>();
    for (const group of suiteGroups) {
      const label = groupSectionLabel(group.kind, t);
      const groups = sections.get(label) ?? [];
      groups.push(group);
      sections.set(label, groups);
    }
    return [...sections.entries()];
  }, [suiteGroups, t]);

  const hasUnsavedChanges = useMemo(
    () => !isSameKeySet(draftEnabledKeys, committedEnabledKeys),
    [committedEnabledKeys, draftEnabledKeys],
  );

  const isSaving = savingAction !== null || resettingModeId !== null;

  const loadModeSkills = useCallback(async (forceRefresh?: boolean) => {
    const requestId = ++loadRequestIdRef.current;

    try {
      setLoading(true);
      setError(null);
      const skills = await configAPI.getModeSkillConfigs({
        modeId: suiteModeId,
        forceRefresh,
        workspacePath: workspacePath || undefined,
      });

      if (requestId !== loadRequestIdRef.current) {
        return;
      }

      setModeSkills(skills);
      const enabledKeys = buildEnabledKeySet(skills);
      setCommittedEnabledKeys(enabledKeys);
      setDraftEnabledKeys(enabledKeys);
    } catch (loadError) {
      if (requestId !== loadRequestIdRef.current) {
        return;
      }

      const message = loadError instanceof Error ? loadError.message : String(loadError);
      log.error('Failed to load skill suite mode configs', {
        modeId: suiteModeId,
        workspacePath,
        error: loadError,
      });
      setError(message);
    } finally {
      if (requestId === loadRequestIdRef.current) {
        setLoading(false);
      }
    }
  }, [suiteModeId, workspacePath]);

  useEffect(() => {
    void loadModeSkills();
  }, [loadModeSkills]);

  useGallerySceneAutoRefresh({
    sceneId: 'skills',
    refetch: () => loadModeSkills(true),
    enabled: !hasUnsavedChanges,
  });

  const refresh = useCallback(async () => {
    if (hasUnsavedChanges) {
      notification.warning(t('suite.messages.saveFirst'));
      return;
    }
    try {
      await loadModeSkills(true);
    } catch (refreshError) {
      notification.error(
        t('suite.messages.refreshFailed', {
          error: refreshError instanceof Error ? refreshError.message : String(refreshError),
        }),
      );
    }
  }, [hasUnsavedChanges, loadModeSkills, notification, t]);

  const handleModeSelect = useCallback((modeId: typeof SUITE_MODES[number]['id']) => {
    if (hasUnsavedChanges) {
      notification.warning(t('suite.messages.saveFirst'));
      return;
    }

    setSuiteModeId(modeId);
  }, [hasUnsavedChanges, notification, setSuiteModeId, t]);

  const resetMode = useCallback(async (mode: SuiteMode) => {
    const shouldReset = await confirmDialog({
      title: t('suite.resetDialog.title', { mode: t(mode.labelKey) }),
      message: t(
        mode.id === suiteModeId && hasUnsavedChanges
          ? 'suite.resetDialog.messageWithUnsaved'
          : 'suite.resetDialog.message',
        { mode: t(mode.labelKey) },
      ),
      confirmText: t('suite.resetDialog.confirm'),
      cancelText: t('suite.resetDialog.cancel'),
      confirmDanger: true,
      type: 'warning',
    });

    if (!shouldReset) {
      return;
    }

    setResettingModeId(mode.id);

    try {
      await configAPI.resetModeSkillSelection({
        modeId: mode.id,
        workspacePath: workspacePath || undefined,
      });

      if (mode.id === suiteModeId) {
        await loadModeSkills(true);
      }

      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      notification.success(t('suite.messages.resetSuccess', { mode: t(mode.labelKey) }));
    } catch (resetError) {
      log.error('Failed to reset skill suite visibility', {
        modeId: mode.id,
        workspacePath,
        error: resetError,
      });
      notification.error(t('suite.messages.resetFailed', {
        error: resetError instanceof Error ? resetError.message : String(resetError),
      }));
    } finally {
      setResettingModeId(null);
    }
  }, [hasUnsavedChanges, loadModeSkills, notification, suiteModeId, t, workspacePath]);

  const saveGroup = useCallback(async (group: SuiteSkillGroup) => {
    setSavingAction({ groupKey: group.id, kind: 'save' });
    const nextCommitted = uniqueKeys(draftEnabledKeys);

    try {
      await configAPI.replaceModeSkillSelection({
        modeId: suiteModeId,
        enabledSkillKeys: nextCommitted,
        workspacePath: workspacePath || undefined,
      });

      const refreshedSkills = await configAPI.getModeSkillConfigs({
        modeId: suiteModeId,
        forceRefresh: true,
        workspacePath: workspacePath || undefined,
      });
      setModeSkills(refreshedSkills);
      const refreshedEnabledKeys = buildEnabledKeySet(refreshedSkills);
      setCommittedEnabledKeys(refreshedEnabledKeys);
      setDraftEnabledKeys(refreshedEnabledKeys);

      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');

      notification.success(
        t('suite.messages.saveSuccess', {
          mode: t(currentMode.labelKey),
        }),
      );
    } catch (saveError) {
      log.error('Failed to update skill suite visibility', {
        modeId: suiteModeId,
        groupKey: group.id,
        workspacePath,
        error: saveError,
      });
      notification.error(
        t('suite.messages.saveFailed', {
          error: saveError instanceof Error ? saveError.message : String(saveError),
        }),
      );
    } finally {
      setSavingAction(null);
    }
  }, [currentMode.labelKey, draftEnabledKeys, notification, suiteModeId, t, workspacePath]);

  const saveGroupVisibility = useCallback(async (group: SuiteSkillGroup, enabled: boolean) => {
    const groupKeys = buildGroupKeySet(group);
    const previousDraft = draftEnabledKeys;
    const baseDraft = draftEnabledKeys.filter((key) => !groupKeys.has(key));
    const finalDraft = enabled
      ? uniqueKeys([...baseDraft, ...group.skills.map((skill) => skill.key)])
      : uniqueKeys(baseDraft);
    setSavingAction({ groupKey: group.id, kind: 'toggle' });
    setDraftEnabledKeys(finalDraft);
    try {
      await configAPI.replaceModeSkillSelection({
        modeId: suiteModeId,
        enabledSkillKeys: finalDraft,
        workspacePath: workspacePath || undefined,
      });
      const refreshedSkills = await configAPI.getModeSkillConfigs({
        modeId: suiteModeId,
        forceRefresh: true,
        workspacePath: workspacePath || undefined,
      });
      setModeSkills(refreshedSkills);
      const refreshedEnabledKeys = buildEnabledKeySet(refreshedSkills);
      setCommittedEnabledKeys(refreshedEnabledKeys);
      setDraftEnabledKeys(refreshedEnabledKeys);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      notification.success(t('suite.messages.saveSuccess', { mode: t(currentMode.labelKey) }));
    } catch (saveError) {
      log.error('Failed to update skill suite visibility', {
        modeId: suiteModeId,
        groupKey: group.id,
        workspacePath,
        error: saveError,
      });
      notification.error(t('suite.messages.saveFailed', {
        error: saveError instanceof Error ? saveError.message : String(saveError),
      }));
      setDraftEnabledKeys(previousDraft);
    } finally {
      setSavingAction(null);
    }
  }, [currentMode.labelKey, draftEnabledKeys, notification, suiteModeId, t, workspacePath]);

  return (
    <div className="skills-suite" data-openbitfun-scene="skills" data-openbitfun-part="suite" data-openbitfun-mode={suiteModeId}>
      <div className="skills-suite__hero" data-openbitfun-scene="skills" data-openbitfun-part="suiteHero">
        <div className="skills-suite__hero-copy" data-openbitfun-scene="skills" data-openbitfun-part="suiteHeroCopy">
          <h2 className="skills-suite__title">{t('suite.title')}</h2>
          <p className="skills-suite__subtitle">{t('suite.subtitle')}</p>
        </div>
        <div className="skills-suite__hero-actions" data-openbitfun-scene="skills" data-openbitfun-part="suiteHeroActions">
          <Button
            variant="outline"
            size="sm"
            leadingIcon={<Icon name="settings" size="xs" />}
            onClick={() => setIsGroupManagerOpen(true)}
            disabled={isSaving}
          >
            {t('suite.manageGroups')}
          </Button>
          <Button
            variant="outline"
            size="sm"
            leadingIcon={<Icon name="refresh" size="lg" style={{ width: 13, height: 13 }} />}
            onClick={() => void refresh()}
            title={t('suite.refreshTooltip')}
            aria-label={t('suite.refreshTooltip')}
            disabled={loading || isSaving || hasUnsavedChanges}
          >
            {t('suite.refreshAction')}
          </Button>
        </div>
      </div>

      <div className="skills-suite__mode-toolbar" data-openbitfun-scene="skills" data-openbitfun-part="suiteModeToolbar">
        <div className="skills-suite__modes" role="tablist" aria-label={t('suite.modeLabel')} data-openbitfun-scene="skills" data-openbitfun-part="suiteModes">
        {SUITE_MODES.map((mode) => (
            <button
              key={mode.id}
              id={`skills-suite-tab-${mode.id}`}
              type="button"
              role="tab"
              aria-selected={suiteModeId === mode.id}
              aria-controls={`skills-suite-panel-${mode.id}`}
              className={`skills-suite__mode-tab${suiteModeId === mode.id ? ' is-active' : ''}`}
              onClick={() => handleModeSelect(mode.id)}
              disabled={isSaving}
              title={t(mode.descKey)}
              data-openbitfun-scene="skills"
              data-openbitfun-part="suiteModeTab"
              data-openbitfun-mode={mode.id}
              data-openbitfun-state={suiteModeId === mode.id ? 'active' : undefined}
            >
              <span className="skills-suite__mode-tab-label" data-openbitfun-scene="skills" data-openbitfun-part="suiteModeTabLabel">{t(mode.labelKey)}</span>
            </button>
        ))}
        </div>
        <IconButton
          size="sm"
          className="skills-suite__mode-reset"
          loading={resettingModeId === suiteModeId}
          disabled={isSaving}
          onClick={() => { void resetMode(currentMode); }}
          title={t('suite.modeActions.reset', { mode: t(currentMode.labelKey) })}
          aria-label={t('suite.modeActions.reset', { mode: t(currentMode.labelKey) })}
          icon={<RotateCcw size={13} />}
        />
      </div>

      {loading && (
        <div className="skills-suite__loading" aria-busy="true" aria-label={t('suite.loading')} data-openbitfun-scene="skills" data-openbitfun-part="suiteLoading">
          <Icon name="refresh" size="md" className="skills-suite__loading-icon" />
          <span>{t('suite.loading')}</span>
        </div>
      )}

      {!loading && error && (
        <div className="skills-main__empty skills-main__empty--error" data-openbitfun-scene="skills" data-openbitfun-part="error">
          <Package size={28} strokeWidth={1.2} />
          <span>{error}</span>
        </div>
      )}

      {!loading && !error && suiteGroups.length === 0 && (
        <div className="skills-main__empty" data-openbitfun-scene="skills" data-openbitfun-part="empty">
          <Package size={28} strokeWidth={1.2} />
          <span>{t('suite.empty')}</span>
        </div>
      )}

      {!loading && !error && suiteGroups.length > 0 && (
        <ScrollArea
          id={`skills-suite-panel-${suiteModeId}`}
          role="tabpanel"
          aria-labelledby={`skills-suite-tab-${suiteModeId}`}
          className="skills-suite__sections"
        >
          {suiteSections.map(([sectionLabel, sectionGroups]) => (
            <section key={sectionLabel} className="skills-suite__section" data-openbitfun-scene="skills" data-openbitfun-part="suiteSection">
              <span className="skills-suite__section-label" data-openbitfun-scene="skills" data-openbitfun-part="suiteSectionLabel">{sectionLabel}</span>
              <div className="skills-suite__grid" data-openbitfun-scene="skills" data-openbitfun-part="suiteGrid">
                {sectionGroups.map((group) => {
                  const allEnabled = group.enabledCount === group.totalCount;
                  const someEnabled = group.enabledCount > 0;
                  const groupDirty = group.skills.some(
                    (skill) => committedEnabledKeySet.has(skill.key) !== draftEnabledKeySet.has(skill.key),
                  );
                  const showSaveButton = groupDirty
                    && !(savingAction?.groupKey === group.id && savingAction.kind === 'toggle');
                  const groupStateVariant = allEnabled ? 'success' : someEnabled ? 'warning' : 'neutral';
                  const groupStateLabel = allEnabled
                    ? t('suite.groupState.enabled')
                    : someEnabled
                      ? t('suite.groupState.partial')
                      : t('suite.groupState.disabled');

                  return (
                    <section
                      key={group.id}
                      className="skills-suite__group-card"
                      data-openbitfun-scene="skills"
                      data-openbitfun-part="suiteGroupCard"
                      data-openbitfun-state={allEnabled ? 'enabled' : undefined}
                    >
                      <div className="skills-suite__group-head" data-openbitfun-scene="skills" data-openbitfun-part="suiteGroupHead">
                        <div className="skills-suite__group-title-wrap" data-openbitfun-scene="skills" data-openbitfun-part="suiteGroupTitleWrap">
                          <div className="skills-suite__group-title-row" data-openbitfun-scene="skills" data-openbitfun-part="suiteGroupTitleRow">
                            <span className="skills-suite__group-title" data-openbitfun-scene="skills" data-openbitfun-part="suiteGroupTitle">{group.label}</span>
                            <StatusPill tone={groupStateVariant}>{groupStateLabel}</StatusPill>
                          </div>
                          <span className="skills-suite__group-count" data-openbitfun-scene="skills" data-openbitfun-part="suiteGroupCount">
                            {t('suite.groupCount', { total: group.totalCount })}
                          </span>
                        </div>

                        <div className="skills-suite__group-actions" data-openbitfun-scene="skills" data-openbitfun-part="suiteGroupActions">
                          {showSaveButton ? (
                            <Button
                              variant="fill"
                              size="sm"
                              loading={savingAction?.groupKey === group.id && savingAction.kind === 'save'}
                              disabled={isSaving}
                              onClick={() => void saveGroup(group)}
                            >
                              {t('suite.groupActions.save')}
                            </Button>
                          ) : null}
                          <Button
                            variant={allEnabled ? 'outline' : 'fill'}
                            size="sm"
                            loading={savingAction?.groupKey === group.id && savingAction.kind === 'toggle'}
                            disabled={isSaving}
                            onClick={() => void saveGroupVisibility(group, !allEnabled)}
                          >
                            {allEnabled ? t('suite.groupActions.disableGroup') : t('suite.groupActions.enableGroup')}
                          </Button>
                        </div>
                      </div>

                      <div className="skills-suite__skills" data-openbitfun-scene="skills" data-openbitfun-part="suiteSkills">
                        {group.skills.map((skill) => {
                          const draftEnabled = draftEnabledKeySet.has(skill.key);
                          const dirty = committedEnabledKeySet.has(skill.key) !== draftEnabled;
                          const coverageSource = coverageSourceBySkillKey.get(skill.key)
                            ?? t('list.item.unknownSource');
                          const shadowed = draftEnabled && !dirty && coverageSourceBySkillKey.has(skill.key);
                          const accessibleStatus = buildSkillTitle(
                            skill,
                            draftEnabled,
                            shadowed,
                            dirty,
                            coverageSource,
                            t,
                          );

                          return (
                            <button
                              type="button"
                              key={skill.key}
                              className={[
                                'skills-suite__skill-chip',
                                draftEnabled ? 'is-enabled' : 'is-disabled',
                                shadowed ? 'is-shadowed' : '',
                                dirty ? 'is-dirty' : '',
                                !skill.globallyEnabled ? 'is-globally-disabled' : '',
                              ].filter(Boolean).join(' ')}
                              title={accessibleStatus}
                              aria-label={`${skill.name}. ${accessibleStatus}`}
                              aria-pressed={draftEnabled}
                              disabled={isSaving}
                              data-openbitfun-scene="skills"
                              data-openbitfun-part="suiteSkill"
                              data-openbitfun-state={[
                                draftEnabled && 'enabled',
                                shadowed && 'covered',
                                dirty && 'dirty',
                              ].filter(Boolean).join(' ') || undefined}
                              onClick={() => {
                                setDraftEnabledKeys((prev) => {
                                  const next = new Set(prev);
                                  if (next.has(skill.key)) {
                                    next.delete(skill.key);
                                  } else {
                                    next.add(skill.key);
                                  }
                                  return uniqueKeys(next);
                                });
                              }}
                            >
                              <span className="skills-suite__skill-chip-name" data-openbitfun-scene="skills" data-openbitfun-part="suiteSkillName">{skill.name}</span>
                              {draftEnabled && !shadowed ? (
                                <ShieldCheck size={11} />
                              ) : (
                                <ShieldAlert size={11} />
                              )}
                              {shadowed && (
                                <span className="skills-suite__skill-chip-status" data-openbitfun-scene="skills" data-openbitfun-part="suiteSkillStatus">
                                  {t('suite.skillState.covered', { source: coverageSource })}
                                </span>
                              )}
                              {dirty && (
                                <span className="skills-suite__skill-chip-status" data-openbitfun-scene="skills" data-openbitfun-part="suiteSkillStatus">
                                  {t('suite.skillState.pending')}
                                </span>
                              )}
                              {!skill.globallyEnabled && (
                                <span className="skills-suite__skill-chip-status">
                                  {t('suite.skillState.globalDisabled')}
                                </span>
                              )}
                            </button>
                          );
                        })}
                      </div>
                    </section>
                  );
                })}
              </div>
            </section>
          ))}
        </ScrollArea>
      )}
      <SkillGroupManagerModal
        isOpen={isGroupManagerOpen}
        onClose={() => setIsGroupManagerOpen(false)}
        skills={modeSkills}
        groups={userSkillGroups}
        onSaveGroups={saveUserSkillGroups}
      />
    </div>
  );
};

export default SkillsSuiteView;
