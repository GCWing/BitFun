import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Package, RefreshCw, RotateCcw, ShieldAlert, ShieldCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button } from '@/component-library';
import { confirmDialog } from '@/component-library/components/ConfirmDialog/confirmService';
import { configAPI } from '@/infrastructure/api';
import { useWorkspaceManagerSync } from '@/infrastructure/hooks/useWorkspaceManagerSync';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';
import type { ModeSkillInfo } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type { SuiteModeId } from '../skillsSceneStore';
import { useSkillsSceneStore } from '../skillsSceneStore';

const log = createLogger('SkillsSuiteView');

const UNGROUPED_SKILL_GROUP = '__ungrouped__';

const SKILL_GROUP_ORDER: Record<string, number> = {
  meta: 0,
  'computer-use': 1,
  office: 2,
  gstack: 3,
  [UNGROUPED_SKILL_GROUP]: 99,
};

const SUITE_MODES = [
  { id: 'agentic', labelKey: 'suite.modes.agentic', descKey: 'suite.modeDescriptions.agentic' },
  { id: 'Cowork', labelKey: 'suite.modes.cowork', descKey: 'suite.modeDescriptions.cowork' },
  { id: 'Claw', labelKey: 'suite.modes.claw', descKey: 'suite.modeDescriptions.claw' },
  { id: 'Team', labelKey: 'suite.modes.team', descKey: 'suite.modeDescriptions.team' },
] as const;

type SuiteMode = typeof SUITE_MODES[number];

interface SuiteSkillGroup {
  key: string;
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

function getSkillGroupKey(skill: ModeSkillInfo): string {
  return skill.groupKey?.trim() || UNGROUPED_SKILL_GROUP;
}

function getSkillGroupLabel(groupKey: string, t: (key: string) => string): string {
  switch (groupKey) {
    case 'office':
      return t('suite.groups.office');
    case 'computer-use':
      return t('suite.groups.computerUse');
    case 'meta':
      return t('suite.groups.meta');
    case 'gstack':
      return t('suite.groups.gstack');
    default:
      return t('suite.groups.other');
  }
}

function buildBuiltinSkillGroups(
  skills: ModeSkillInfo[],
  enabledKeySet: Set<string>,
  t: (key: string) => string,
): SuiteSkillGroup[] {
  const groups = new Map<string, ModeSkillInfo[]>();

  for (const skill of skills) {
    if (!skill.isBuiltin) {
      continue;
    }

    const groupKey = getSkillGroupKey(skill);
    const items = groups.get(groupKey);
    if (items) {
      items.push(skill);
    } else {
      groups.set(groupKey, [skill]);
    }
  }

  return [...groups.entries()]
    .map(([groupKey, groupSkills]) => ({
      key: groupKey,
      label: getSkillGroupLabel(groupKey, t),
      skills: [...groupSkills].sort((a, b) => {
        const aEnabled = enabledKeySet.has(a.key);
        const bEnabled = enabledKeySet.has(b.key);
        if (aEnabled && !bEnabled) return -1;
        if (!aEnabled && bEnabled) return 1;
        return a.name.localeCompare(b.name) || a.key.localeCompare(b.key);
      }),
      enabledCount: groupSkills.filter((skill) => enabledKeySet.has(skill.key)).length,
      totalCount: groupSkills.length,
    }))
    .sort((a, b) => {
      const orderDiff = (SKILL_GROUP_ORDER[a.key] ?? 50) - (SKILL_GROUP_ORDER[b.key] ?? 50);
      if (orderDiff !== 0) {
        return orderDiff;
      }
      return a.label.localeCompare(b.label);
    });
}

function buildSkillTitle(skill: ModeSkillInfo, enabled: boolean, t: (key: string) => string): string {
  return [
    skill.description || skill.name,
    enabled ? t('suite.groupState.enabled') : t('suite.groupState.disabled'),
    enabled && skill.isShadowed ? t('list.item.shadowedTooltip') : null,
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

  const [modeSkills, setModeSkills] = useState<ModeSkillInfo[]>([]);
  const [committedEnabledKeys, setCommittedEnabledKeys] = useState<string[]>([]);
  const [draftEnabledKeys, setDraftEnabledKeys] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [savingAction, setSavingAction] = useState<SavingAction>(null);
  const [resettingModeId, setResettingModeId] = useState<SuiteModeId | null>(null);
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

  const suiteGroups = useMemo(
    () => buildBuiltinSkillGroups(modeSkills, draftEnabledKeySet, t),
    [modeSkills, draftEnabledKeySet, t],
  );

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
    setSavingAction({ groupKey: group.key, kind: 'save' });
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
        groupKey: group.key,
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
    setSavingAction({ groupKey: group.key, kind: 'toggle' });
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
        groupKey: group.key,
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
    <div className="skills-suite">
      <div className="skills-suite__hero">
        <div className="skills-suite__hero-copy">
          <h2 className="skills-suite__title">{t('suite.title')}</h2>
          <p className="skills-suite__subtitle">{t('suite.subtitle')}</p>
        </div>
        <Button
          variant="secondary"
          size="small"
          onClick={() => void refresh()}
          title={t('suite.refreshTooltip')}
          aria-label={t('suite.refreshTooltip')}
          disabled={loading || isSaving || hasUnsavedChanges}
        >
          <RefreshCw size={13} />
          <span>{t('suite.refreshAction')}</span>
        </Button>
      </div>

      <div className="skills-suite__mode-toolbar">
        <div className="skills-suite__modes" role="tablist" aria-label={t('suite.modeLabel')}>
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
            >
              <span className="skills-suite__mode-tab-label">{t(mode.labelKey)}</span>
            </button>
        ))}
        </div>
        <Button
          variant="secondary"
          size="small"
          className="skills-suite__mode-reset"
          iconOnly
          isLoading={resettingModeId === suiteModeId}
          disabled={isSaving}
          onClick={() => { void resetMode(currentMode); }}
          title={t('suite.modeActions.reset', { mode: t(currentMode.labelKey) })}
          aria-label={t('suite.modeActions.reset', { mode: t(currentMode.labelKey) })}
        >
          <RotateCcw size={13} />
        </Button>
      </div>

      {loading && (
        <div className="skills-suite__loading" aria-busy="true" aria-label={t('suite.loading')}>
          <RefreshCw size={16} className="skills-suite__loading-icon" />
          <span>{t('suite.loading')}</span>
        </div>
      )}

      {!loading && error && (
        <div className="skills-main__empty skills-main__empty--error">
          <Package size={28} strokeWidth={1.2} />
          <span>{error}</span>
        </div>
      )}

      {!loading && !error && suiteGroups.length === 0 && (
        <div className="skills-main__empty">
          <Package size={28} strokeWidth={1.2} />
          <span>{t('suite.empty')}</span>
        </div>
      )}

      {!loading && !error && suiteGroups.length > 0 && (
        <div
          id={`skills-suite-panel-${suiteModeId}`}
          role="tabpanel"
          aria-labelledby={`skills-suite-tab-${suiteModeId}`}
          className="skills-suite__grid"
        >
          {suiteGroups.map((group) => {
            const allEnabled = group.enabledCount === group.totalCount;
            const someEnabled = group.enabledCount > 0;
            const groupDirty = group.skills.some(
              (skill) => committedEnabledKeySet.has(skill.key) !== draftEnabledKeySet.has(skill.key),
            );
            const showSaveButton = groupDirty
              && !(savingAction?.groupKey === group.key && savingAction.kind === 'toggle');
            const groupStateVariant = allEnabled ? 'success' : someEnabled ? 'warning' : 'neutral';
            const groupStateLabel = allEnabled
              ? t('suite.groupState.enabled')
              : someEnabled
                ? t('suite.groupState.partial')
                : t('suite.groupState.disabled');

            return (
              <section key={group.key} className="skills-suite__group-card">
                <div className="skills-suite__group-head">
                  <div className="skills-suite__group-title-wrap">
                    <div className="skills-suite__group-title-row">
                      <span className="skills-suite__group-title">{group.label}</span>
                      <Badge variant={groupStateVariant}>{groupStateLabel}</Badge>
                    </div>
                    <span className="skills-suite__group-count">
                      {t('suite.groupCount', { total: group.totalCount })}
                    </span>
                  </div>

                  <div className="skills-suite__group-actions">
                    {showSaveButton ? (
                      <Button
                        variant="primary"
                        size="small"
                        isLoading={savingAction?.groupKey === group.key && savingAction.kind === 'save'}
                        disabled={isSaving}
                        onClick={() => void saveGroup(group)}
                      >
                        {t('suite.groupActions.save')}
                      </Button>
                    ) : null}
                    <Button
                      variant={allEnabled ? 'secondary' : 'primary'}
                      size="small"
                      isLoading={savingAction?.groupKey === group.key && savingAction.kind === 'toggle'}
                      disabled={isSaving}
                      onClick={() => void saveGroupVisibility(group, !allEnabled)}
                    >
                      {allEnabled ? t('suite.groupActions.disableGroup') : t('suite.groupActions.enableGroup')}
                    </Button>
                  </div>
                </div>

                <div className="skills-suite__skills">
                  {group.skills.map((skill) => {
                    const draftEnabled = draftEnabledKeySet.has(skill.key);
                    const dirty = committedEnabledKeySet.has(skill.key) !== draftEnabled;
                    const shadowed = draftEnabled && skill.isShadowed;

                    return (
                      <button
                        type="button"
                        key={skill.key}
                        className={[
                          'skills-suite__skill-chip',
                          draftEnabled ? 'is-enabled' : 'is-disabled',
                          shadowed ? 'is-shadowed' : '',
                          dirty ? 'is-dirty' : '',
                        ].filter(Boolean).join(' ')}
                        title={buildSkillTitle(skill, draftEnabled, t)}
                        disabled={isSaving}
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
                        <span className="skills-suite__skill-chip-name">{skill.name}</span>
                        {draftEnabled ? (
                          <ShieldCheck size={11} />
                        ) : (
                          <ShieldAlert size={11} />
                        )}
                      </button>
                    );
                  })}
                </div>
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default SkillsSuiteView;
