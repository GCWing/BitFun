import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Package, RefreshCw, RotateCcw, Settings2, ShieldAlert, ShieldCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button } from '@/component-library';
import { confirmDialog } from '@/component-library/components/ConfirmDialog/confirmService';
import { configAPI } from '@/infrastructure/api';
import { useWorkspaceManagerSync } from '@/infrastructure/hooks/useWorkspaceManagerSync';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type { UserToolGroup } from '@/infrastructure/config/types';
import {
  type GroupableTool,
  type ResolvedToolGroup,
  resolveToolGroups,
} from './toolGroups';
import { GroupManagerModal as ToolGroupManagerModal } from './ToolGroupPicker';
import '../../skills/SkillsScene.scss';

const log = createLogger('ToolSuiteView');

const SUITE_MODES = [
  { id: 'agentic', labelKey: 'suite.modes.agentic', descKey: 'suite.modeDescriptions.agentic' },
  { id: 'Cowork', labelKey: 'suite.modes.cowork', descKey: 'suite.modeDescriptions.cowork' },
  { id: 'Claw', labelKey: 'shared:agents.claw', descKey: 'suite.modeDescriptions.claw' },
  { id: 'Team', labelKey: 'suite.modes.team', descKey: 'suite.modeDescriptions.team' },
] as const;

type SuiteMode = typeof SUITE_MODES[number];

interface SuiteToolGroup {
  id: string;
  kind: ResolvedToolGroup['kind'];
  label: string;
  tools: GroupableTool[];
  enabledCount: number;
  totalCount: number;
}

type SavingAction = {
  groupKey: string;
  kind: 'save' | 'toggle';
} | null;

function uniqueNames(names: Iterable<string>): string[] {
  return [...new Set([...names].filter(Boolean))];
}

function cloneSet(names: Iterable<string>): Set<string> {
  return new Set(names);
}

function groupSectionLabel(kind: ResolvedToolGroup['kind'], t: (key: string) => string): string {
  switch (kind) {
    case 'user':
      return t('agentsOverview.toolGroups.myGroups');
    case 'extension':
      return t('agentsOverview.toolGroups.extensions');
    case 'other':
      return t('agentsOverview.toolGroups.otherTools');
    default:
      return t('agentsOverview.toolGroups.builtin');
  }
}

function isSameNameSet(leftNames: string[], rightNames: string[]): boolean {
  if (leftNames.length !== rightNames.length) {
    return false;
  }
  const rightSet = new Set(rightNames);
  return leftNames.every((name) => rightSet.has(name));
}

function buildGroupKeySet(group: SuiteToolGroup): Set<string> {
  return new Set(group.tools.map((tool) => tool.name));
}

function buildToolTitle(tool: GroupableTool, enabled: boolean, dirty: boolean): string {
  return [
    tool.description || tool.name,
    dirty
      ? 'Pending changes'
      : enabled
        ? 'Enabled for this mode'
        : 'Disabled for this mode',
  ].filter(Boolean).join('\n');
}

interface ToolSuiteViewProps {
  /** All selectable tools from the live registry. */
  tools: GroupableTool[];
  /** Resolve a mode's enabled + default tool names (from agent profile config). */
  getModeConfig: (modeId: string) => {
    enabled_tools: string[];
    default_tools: string[];
  } | null;
  userGroups: UserToolGroup[];
  onSaveUserGroups: (groups: UserToolGroup[]) => Promise<void>;
}

const ToolSuiteView: React.FC<ToolSuiteViewProps> = ({
  tools,
  getModeConfig,
  userGroups,
  onSaveUserGroups,
}) => {
  const { t } = useTranslation('scenes/agents');
  const notification = useNotification();
  const { workspacePath } = useWorkspaceManagerSync();
  const [suiteModeId, setSuiteModeId] = useState<SuiteMode['id']>('agentic');
  const [committedEnabledNames, setCommittedEnabledNames] = useState<string[]>([]);
  const [draftEnabledNames, setDraftEnabledNames] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [savingAction, setSavingAction] = useState<SavingAction>(null);
  const [resettingModeId, setResettingModeId] = useState<SuiteMode['id'] | null>(null);
  const [isGroupManagerOpen, setIsGroupManagerOpen] = useState(false);
  const loadRequestIdRef = useRef(0);

  const currentMode = useMemo(
    () => SUITE_MODES.find((mode) => mode.id === suiteModeId) ?? SUITE_MODES[0],
    [suiteModeId],
  );

  const committedEnabledNameSet = useMemo(
    () => cloneSet(committedEnabledNames),
    [committedEnabledNames],
  );
  const draftEnabledNameSet = useMemo(
    () => cloneSet(draftEnabledNames),
    [draftEnabledNames],
  );

  const suiteGroups = useMemo(() => {
    const enabledSet = draftEnabledNameSet;
    return resolveToolGroups(tools, userGroups, t).map((group) => {
      const groupTools = [...group.tools].sort((left, right) => {
        const leftEnabled = enabledSet.has(left.name);
        const rightEnabled = enabledSet.has(right.name);
        if (leftEnabled && !rightEnabled) return -1;
        if (!leftEnabled && rightEnabled) return 1;
        return left.name.localeCompare(right.name);
      });
      return {
        id: group.id,
        kind: group.kind,
        label: group.label,
        tools: groupTools,
        enabledCount: groupTools.filter((tool) => enabledSet.has(tool.name)).length,
        totalCount: groupTools.length,
      };
    });
  }, [draftEnabledNameSet, tools, userGroups, t]);

  const suiteSections = useMemo(() => {
    const sections = new Map<string, SuiteToolGroup[]>();
    for (const group of suiteGroups) {
      const label = groupSectionLabel(group.kind, t);
      const groups = sections.get(label) ?? [];
      groups.push(group);
      sections.set(label, groups);
    }
    return [...sections.entries()];
  }, [suiteGroups, t]);

  const hasUnsavedChanges = useMemo(
    () => !isSameNameSet(draftEnabledNames, committedEnabledNames),
    [committedEnabledNames, draftEnabledNames],
  );

  const isSaving = savingAction !== null || resettingModeId !== null;

  const loadModeTools = useCallback(async (_forceRefresh?: boolean) => {
    const requestId = ++loadRequestIdRef.current;
    try {
      setLoading(true);
      setError(null);
      const config = getModeConfig(suiteModeId);
      if (!config) {
        if (requestId === loadRequestIdRef.current) {
          setCommittedEnabledNames([]);
          setDraftEnabledNames([]);
        }
        return;
      }
      if (requestId !== loadRequestIdRef.current) {
        return;
      }
      setCommittedEnabledNames(config.enabled_tools);
      setDraftEnabledNames(config.enabled_tools);
    } catch (loadError) {
      if (requestId !== loadRequestIdRef.current) {
        return;
      }
      const message = loadError instanceof Error ? loadError.message : String(loadError);
      log.error('Failed to load tool suite mode configs', {
        modeId: suiteModeId,
        error: loadError,
      });
      setError(message);
    } finally {
      if (requestId === loadRequestIdRef.current) {
        setLoading(false);
      }
    }
  }, [getModeConfig, suiteModeId]);

  useEffect(() => {
    void loadModeTools();
  }, [loadModeTools]);

  useGallerySceneAutoRefresh({
    sceneId: 'skills',
    refetch: () => loadModeTools(true),
    enabled: !hasUnsavedChanges,
  });

  const refresh = useCallback(async () => {
    if (hasUnsavedChanges) {
      notification.warning(t('agentsOverview.toolGroups.saveFirst'));
      return;
    }
    try {
      await loadModeTools(true);
    } catch (refreshError) {
      notification.error(
        t('agentsOverview.toolGroups.refreshFailed', {
          error: refreshError instanceof Error ? refreshError.message : String(refreshError),
        }),
      );
    }
  }, [hasUnsavedChanges, loadModeTools, notification, t]);

  const handleModeSelect = useCallback((modeId: SuiteMode['id']) => {
    if (hasUnsavedChanges) {
      notification.warning(t('agentsOverview.toolGroups.saveFirst'));
      return;
    }
    setSuiteModeId(modeId);
  }, [hasUnsavedChanges, notification, t]);

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
      await configAPI.resetModeToolSelection({
        modeId: mode.id,
        workspacePath: workspacePath || undefined,
      });

      if (mode.id === suiteModeId) {
        await loadModeTools(true);
      }

      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      notification.success(t('suite.messages.resetSuccess', { mode: t(mode.labelKey) }));
    } catch (resetError) {
      log.error('Failed to reset tool suite visibility', {
        modeId: mode.id,
        error: resetError,
      });
      notification.error(t('suite.messages.resetFailed', {
        error: resetError instanceof Error ? resetError.message : String(resetError),
      }));
    } finally {
      setResettingModeId(null);
    }
  }, [hasUnsavedChanges, loadModeTools, notification, suiteModeId, t, workspacePath]);

  const saveGroup = useCallback(async (group: SuiteToolGroup) => {
    setSavingAction({ groupKey: group.id, kind: 'save' });
    const nextCommitted = uniqueNames(draftEnabledNames);

    try {
      await configAPI.replaceModeToolSelection({
        modeId: suiteModeId,
        enabledToolNames: nextCommitted,
        workspacePath: workspacePath || undefined,
      });

      setCommittedEnabledNames(nextCommitted);
      setDraftEnabledNames(nextCommitted);

      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');

      notification.success(
        t('suite.messages.saveSuccess', {
          mode: t(currentMode.labelKey),
        }),
      );
    } catch (saveError) {
      log.error('Failed to update tool suite visibility', {
        modeId: suiteModeId,
        groupKey: group.id,
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
  }, [currentMode.labelKey, draftEnabledNames, notification, suiteModeId, t, workspacePath]);

  const saveGroupVisibility = useCallback(async (group: SuiteToolGroup, enabled: boolean) => {
    const groupKeys = buildGroupKeySet(group);
    const previousDraft = draftEnabledNames;
    const baseDraft = draftEnabledNames.filter((name) => !groupKeys.has(name));
    const finalDraft = enabled
      ? uniqueNames([...baseDraft, ...group.tools.map((tool) => tool.name)])
      : uniqueNames(baseDraft);
    setSavingAction({ groupKey: group.id, kind: 'toggle' });
    setDraftEnabledNames(finalDraft);
    try {
      await configAPI.replaceModeToolSelection({
        modeId: suiteModeId,
        enabledToolNames: finalDraft,
        workspacePath: workspacePath || undefined,
      });
      setCommittedEnabledNames(finalDraft);
      setDraftEnabledNames(finalDraft);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      notification.success(t('suite.messages.saveSuccess', { mode: t(currentMode.labelKey) }));
    } catch (saveError) {
      log.error('Failed to update tool suite visibility', {
        modeId: suiteModeId,
        groupKey: group.id,
        error: saveError,
      });
      notification.error(t('suite.messages.saveFailed', {
        error: saveError instanceof Error ? saveError.message : String(saveError),
      }));
      setDraftEnabledNames(previousDraft);
    } finally {
      setSavingAction(null);
    }
  }, [currentMode.labelKey, draftEnabledNames, notification, suiteModeId, t, workspacePath]);

  return (
    <div className="skills-suite" data-bf-scene="tools" data-bf-part="toolSuite" data-bf-mode={suiteModeId}>
      <div className="skills-suite__hero" data-bf-scene="tools" data-bf-part="toolSuiteHero">
        <div className="skills-suite__hero-copy" data-bf-scene="tools" data-bf-part="toolSuiteHeroCopy">
          <h2 className="skills-suite__title">{t('agentsOverview.toolGroups.suiteTitle')}</h2>
          <p className="skills-suite__subtitle">{t('agentsOverview.toolGroups.suiteSubtitle')}</p>
        </div>
        <div className="skills-suite__hero-actions" data-bf-scene="tools" data-bf-part="toolSuiteHeroActions">
          <Button
            variant="secondary"
            size="small"
            onClick={() => setIsGroupManagerOpen(true)}
            disabled={isSaving}
          >
            <Settings2 size={13} />
            <span>{t('agentsOverview.toolGroups.manageGroups')}</span>
          </Button>
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
      </div>

      <div className="skills-suite__mode-toolbar" data-bf-scene="tools" data-bf-part="toolSuiteModeToolbar">
        <div className="skills-suite__modes" role="tablist" aria-label={t('suite.modeLabel')} data-bf-scene="tools" data-bf-part="toolSuiteModes">
          {SUITE_MODES.map((mode) => (
            <button
              key={mode.id}
              id={`tool-suite-tab-${mode.id}`}
              type="button"
              role="tab"
              aria-selected={suiteModeId === mode.id}
              aria-controls={`tool-suite-panel-${mode.id}`}
              className={`skills-suite__mode-tab${suiteModeId === mode.id ? ' is-active' : ''}`}
              onClick={() => handleModeSelect(mode.id)}
              disabled={isSaving}
              title={t(mode.descKey)}
              data-bf-scene="tools"
              data-bf-part="toolSuiteModeTab"
              data-bf-mode={mode.id}
              data-bf-state={suiteModeId === mode.id ? 'active' : undefined}
            >
              <span className="skills-suite__mode-tab-label" data-bf-scene="tools" data-bf-part="toolSuiteModeTabLabel">{t(mode.labelKey)}</span>
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
        <div className="skills-suite__loading" aria-busy="true" aria-label={t('suite.loading')} data-bf-scene="tools" data-bf-part="toolSuiteLoading">
          <RefreshCw size={16} className="skills-suite__loading-icon" />
          <span>{t('suite.loading')}</span>
        </div>
      )}

      {!loading && error && (
        <div className="skills-main__empty skills-main__empty--error" data-bf-scene="tools" data-bf-part="toolSuiteError">
          <Package size={28} strokeWidth={1.2} />
          <span>{error}</span>
        </div>
      )}

      {!loading && !error && suiteGroups.length === 0 && (
        <div className="skills-main__empty" data-bf-scene="tools" data-bf-part="toolSuiteEmpty">
          <Package size={28} strokeWidth={1.2} />
          <span>{t('suite.empty')}</span>
        </div>
      )}

      {!loading && !error && suiteGroups.length > 0 && (
        <div
          id={`tool-suite-panel-${suiteModeId}`}
          role="tabpanel"
          aria-labelledby={`tool-suite-tab-${suiteModeId}`}
          className="skills-suite__sections"
          data-bf-scene="tools"
          data-bf-part="toolSuiteSections"
          data-bf-mode={suiteModeId}
        >
          {suiteSections.map(([sectionLabel, sectionGroups]) => (
            <section key={sectionLabel} className="skills-suite__section" data-bf-scene="tools" data-bf-part="toolSuiteSection">
              <span className="skills-suite__section-label" data-bf-scene="tools" data-bf-part="toolSuiteSectionLabel">{sectionLabel}</span>
              <div className="skills-suite__grid" data-bf-scene="tools" data-bf-part="toolSuiteGrid">
                {sectionGroups.map((group) => {
                  const allEnabled = group.enabledCount === group.totalCount;
                  const someEnabled = group.enabledCount > 0;
                  const groupDirty = group.tools.some(
                    (tool) => committedEnabledNameSet.has(tool.name) !== draftEnabledNameSet.has(tool.name),
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
                      data-bf-scene="tools"
                      data-bf-part="toolSuiteGroupCard"
                      data-bf-state={allEnabled ? 'enabled' : undefined}
                    >
                      <div className="skills-suite__group-head" data-bf-scene="tools" data-bf-part="toolSuiteGroupHead">
                        <div className="skills-suite__group-title-wrap" data-bf-scene="tools" data-bf-part="toolSuiteGroupTitleWrap">
                          <div className="skills-suite__group-title-row" data-bf-scene="tools" data-bf-part="toolSuiteGroupTitleRow">
                            <span className="skills-suite__group-title" data-bf-scene="tools" data-bf-part="toolSuiteGroupTitle">{group.label}</span>
                            <Badge variant={groupStateVariant}>{groupStateLabel}</Badge>
                          </div>
                          <span className="skills-suite__group-count" data-bf-scene="tools" data-bf-part="toolSuiteGroupCount">
                            {t('suite.groupCount', { total: group.totalCount })}
                          </span>
                        </div>

                        <div className="skills-suite__group-actions" data-bf-scene="tools" data-bf-part="toolSuiteGroupActions">
                          {showSaveButton ? (
                            <Button
                              variant="primary"
                              size="small"
                              isLoading={savingAction?.groupKey === group.id && savingAction.kind === 'save'}
                              disabled={isSaving}
                              onClick={() => void saveGroup(group)}
                            >
                              {t('suite.groupActions.save')}
                            </Button>
                          ) : null}
                          <Button
                            variant={allEnabled ? 'secondary' : 'primary'}
                            size="small"
                            isLoading={savingAction?.groupKey === group.id && savingAction.kind === 'toggle'}
                            disabled={isSaving}
                            onClick={() => void saveGroupVisibility(group, !allEnabled)}
                          >
                            {allEnabled ? t('suite.groupActions.disableGroup') : t('suite.groupActions.enableGroup')}
                          </Button>
                        </div>
                      </div>

                      <div className="skills-suite__skills" data-bf-scene="tools" data-bf-part="toolSuiteTools">
                        {group.tools.map((tool) => {
                          const draftEnabled = draftEnabledNameSet.has(tool.name);
                          const dirty = committedEnabledNameSet.has(tool.name) !== draftEnabled;
                          const accessibleStatus = buildToolTitle(tool, draftEnabled, dirty);

                          return (
                            <button
                              type="button"
                              key={tool.name}
                              className={[
                                'skills-suite__skill-chip',
                                draftEnabled ? 'is-enabled' : 'is-disabled',
                                dirty ? 'is-dirty' : '',
                              ].filter(Boolean).join(' ')}
                              title={accessibleStatus}
                              aria-label={`${tool.name}. ${accessibleStatus}`}
                              aria-pressed={draftEnabled}
                              disabled={isSaving}
                              data-bf-scene="tools"
                              data-bf-part="toolSuiteTool"
                              data-bf-state={[
                                draftEnabled && 'enabled',
                                dirty && 'dirty',
                              ].filter(Boolean).join(' ') || undefined}
                              onClick={() => {
                                setDraftEnabledNames((prev) => {
                                  const next = new Set(prev);
                                  if (next.has(tool.name)) {
                                    next.delete(tool.name);
                                  } else {
                                    next.add(tool.name);
                                  }
                                  return uniqueNames(next);
                                });
                              }}
                            >
                              <span className="skills-suite__skill-chip-name" data-bf-scene="tools" data-bf-part="toolSuiteToolName">{tool.name}</span>
                              {draftEnabled ? (
                                <ShieldCheck size={11} />
                              ) : (
                                <ShieldAlert size={11} />
                              )}
                              {dirty && (
                                <span className="skills-suite__skill-chip-status" data-bf-scene="tools" data-bf-part="toolSuiteToolStatus">
                                  {t('suite.skillState.pending')}
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
        </div>
      )}
      <ToolGroupManagerModal
        isOpen={isGroupManagerOpen}
        onClose={() => setIsGroupManagerOpen(false)}
        tools={tools}
        groups={userGroups}
        onSaveGroups={onSaveUserGroups}
      />
    </div>
  );
};

export default ToolSuiteView;
