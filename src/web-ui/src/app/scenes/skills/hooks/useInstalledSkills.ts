import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { configAPI } from '@/infrastructure/api';
import { getActiveSurfaceScope, onSurfaceActivated } from '@/infrastructure/peer-device/deviceSurface';
import type { SkillInfo, SkillLevel, SkillValidationResult, SkillScanDiagnostic } from '@/infrastructure/config/types';
import { canDeleteSkill, getSkillSourceId, getSkillSourceLabel } from '@/infrastructure/config/skillSourcePresentation';
import { useWorkspaceManagerSync } from '@/infrastructure/hooks/useWorkspaceManagerSync';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type { InstalledFilter } from '../skillsSceneStore';

const log = createLogger('SkillsScene:useInstalledSkills');

function installedSkillGroup(skill: SkillInfo): InstalledFilter {
  if (skill.isBuiltin) return 'builtin';
  const sourceId = getSkillSourceId(skill);
  return sourceId === 'openbitfun' ? skill.level : `source:${sourceId}`;
}

interface UseInstalledSkillsOptions {
  searchQuery: string;
  activeFilter: InstalledFilter;
  enabled?: boolean;
}

export function useInstalledSkills({
  searchQuery,
  activeFilter,
  enabled = true,
}: UseInstalledSkillsOptions) {
  const { t } = useTranslation('scenes/skills');
  const notification = useNotification();
  const { warning: notifyScanWarning, info: notifyScanInfo } = notification;
  const { workspace, workspacePath, hasWorkspace, isRemoteWorkspace } = useWorkspaceManagerSync();

  const scope = useSyncExternalStore(onSurfaceActivated, getActiveSurfaceScope, getActiveSurfaceScope);
  const [loadedContextKey, setLoadedContextKey] = useState<string | null>(null);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [diagnostics, setDiagnostics] = useState<SkillScanDiagnostic[]>([]);
  const [diagnosticsAvailable, setDiagnosticsAvailable] = useState(true);
  const [globallyDisabledSkillKeys, setGloballyDisabledSkillKeys] = useState<Set<string>>(new Set());
  const [savingGlobalSkillKey, setSavingGlobalSkillKey] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [formLevel, setFormLevel] = useState<SkillLevel>('user');
  const [formPath, setFormPath] = useState('');
  const [validationResult, setValidationResult] = useState<SkillValidationResult | null>(null);
  const [isValidating, setIsValidating] = useState(false);
  const [isAdding, setIsAdding] = useState(false);
  const loadRequestIdRef = useRef(0);
  const lastScanFeedbackKeyRef = useRef<string | null>(null);
  const validationRequestIdRef = useRef(0);
  const capabilityKey = scope.key(scope.epoch, String(enabled), workspace?.id, workspacePath, workspace?.connectionId, String(isRemoteWorkspace));
  const capabilityRef = useRef({ key: capabilityKey, epoch: 0, enabled });
  useLayoutEffect(() => {
    if (capabilityRef.current.key !== capabilityKey) {
      capabilityRef.current = {
        key: capabilityKey,
        epoch: capabilityRef.current.epoch + 1,
        enabled,
      };
    } else {
      capabilityRef.current.enabled = enabled;
    }
  }, [capabilityKey, enabled]);

  const currentCapabilityEpoch = useCallback((): number | null => (
    capabilityRef.current.enabled ? capabilityRef.current.epoch : null
  ), []);
  const capabilityIsCurrent = useCallback((epoch: number): boolean => (
    capabilityRef.current.enabled && capabilityRef.current.epoch === epoch
  ), []);

  const loadSkills = useCallback(async (forceRefresh?: boolean) => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) {
      return;
    }
    const requestId = ++loadRequestIdRef.current;

    try {
      setLoading(true);
      setError(null);
      const [list, globalSettings] = await Promise.all([
        configAPI.getSkillScanReport({
          forceRefresh,
          workspacePath: workspacePath || undefined,
        }),
        configAPI.getGlobalSkillSettings(),
      ]);
      if (requestId !== loadRequestIdRef.current || !capabilityIsCurrent(capabilityEpoch)) {
        return;
      }
      setLoadedContextKey(capabilityRef.current.key);
      setSkills(list.skills);
      setDiagnostics(list.diagnostics);
      setDiagnosticsAvailable(list.diagnosticsAvailable);
      setGloballyDisabledSkillKeys(new Set(globalSettings.globallyDisabledUserSkillKeys));

      const diagnosticKeys = list.diagnostics
        .map(({ sourceId, path, message }) => JSON.stringify([sourceId, path, message]))
        .sort();
      const feedbackKey = JSON.stringify([
        capabilityRef.current.key, list.diagnosticsAvailable, diagnosticKeys,
      ]);
      // Gallery focus and tab re-entry refresh the scan; only changed results notify.
      if (lastScanFeedbackKeyRef.current !== feedbackKey) {
        lastScanFeedbackKeyRef.current = feedbackKey;
        if (list.diagnostics.length > 0) {
          const message = list.skills.length > 0 ? t('list.scanIncomplete') : t('list.loadFailed');
          notifyScanWarning(message, {
            title: t('nav.title'),
            metadata: {
              diagnostics: list.diagnostics
                .map(({ path, message }) => `${path}: ${message}`)
                .join('\n'),
            },
          });
        } else if (!list.diagnosticsAvailable) {
          notifyScanInfo(t('list.diagnosticsUnavailable'), { title: t('nav.title') });
        }
      }
    } catch (err) {
      if (requestId !== loadRequestIdRef.current || !capabilityIsCurrent(capabilityEpoch)) {
        return;
      }
      log.error('Failed to load skills', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (requestId === loadRequestIdRef.current && capabilityIsCurrent(capabilityEpoch)) {
        setLoading(false);
      }
    }
  }, [capabilityIsCurrent, currentCapabilityEpoch, notifyScanInfo, notifyScanWarning, t, workspacePath]);

  useEffect(() => {
    loadRequestIdRef.current += 1;
    validationRequestIdRef.current += 1;
    setValidationResult(null);
    setIsValidating(false);
    setIsAdding(false);
    if (!enabled) {
      setSkills([]);
      setDiagnostics([]);
      setDiagnosticsAvailable(true);
      setGloballyDisabledSkillKeys(new Set());
      setSavingGlobalSkillKey(null);
      setError(null);
      setLoading(false);
      return;
    }
    void loadSkills();
  }, [capabilityKey, enabled, loadSkills]);

  const validatePath = useCallback(async (path: string) => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) {
      return;
    }
    const requestId = ++validationRequestIdRef.current;
    if (!path.trim()) {
      setValidationResult(null);
      return;
    }
    try {
      setIsValidating(true);
      const result = await configAPI.validateSkillPath(path);
      if (
        requestId !== validationRequestIdRef.current
        || !capabilityIsCurrent(capabilityEpoch)
      ) {
        return;
      }
      setValidationResult(result);
    } catch (err) {
      if (
        requestId !== validationRequestIdRef.current
        || !capabilityIsCurrent(capabilityEpoch)
      ) {
        return;
      }
      setValidationResult({
        valid: false,
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      if (
        requestId === validationRequestIdRef.current
        && capabilityIsCurrent(capabilityEpoch)
      ) {
        setIsValidating(false);
      }
    }
  }, [capabilityIsCurrent, currentCapabilityEpoch]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      validatePath(formPath);
    }, 300);
    return () => window.clearTimeout(timer);
  }, [capabilityKey, formPath, validatePath]);

  const handleBrowse = useCallback(async () => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) {
      return;
    }
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('form.path.label'),
      });
      if (selected && capabilityIsCurrent(capabilityEpoch)) {
        setFormPath(selected as string);
      }
    } catch (err) {
      log.error('Failed to open file dialog', err);
    }
  }, [capabilityIsCurrent, currentCapabilityEpoch, t]);

  const resetForm = useCallback(() => {
    setFormPath('');
    setFormLevel('user');
    setValidationResult(null);
  }, []);

  const handleAdd = useCallback(async () => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) {
      return false;
    }
    if (!validationResult?.valid || !formPath.trim()) {
      notification.warning(t('messages.invalidPath'));
      return false;
    }
    if (formLevel === 'project' && !hasWorkspace) {
      notification.warning(t('messages.noWorkspace'));
      return false;
    }
    if (formLevel === 'project' && isRemoteWorkspace) {
      notification.warning('Remote workspaces do not support project skill installation yet.');
      return false;
    }
    try {
      setIsAdding(true);
      await configAPI.addSkill({
        sourcePath: formPath,
        level: formLevel,
        workspacePath: workspacePath || undefined,
      });
      if (!capabilityIsCurrent(capabilityEpoch)) {
        return false;
      }
      notification.success(t('messages.addSuccess', { name: validationResult.name }));
      resetForm();
      await loadSkills(true);
      return capabilityIsCurrent(capabilityEpoch);
    } catch (err) {
      if (!capabilityIsCurrent(capabilityEpoch)) {
        return false;
      }
      notification.error(
        t('messages.addFailed', {
          error: err instanceof Error ? err.message : String(err),
        }),
      );
      return false;
    } finally {
      if (capabilityIsCurrent(capabilityEpoch)) {
        setIsAdding(false);
      }
    }
  }, [capabilityIsCurrent, currentCapabilityEpoch, formLevel, formPath, hasWorkspace, isRemoteWorkspace, loadSkills, notification, resetForm, t, validationResult, workspacePath]);

  const handleDelete = useCallback(async (skill: SkillInfo) => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) {
      return false;
    }
    if (!canDeleteSkill(skill)) {
      return false;
    }
    try {
      await configAPI.deleteSkill({
        skillKey: skill.key,
        workspacePath: workspacePath || undefined,
      });
      if (!capabilityIsCurrent(capabilityEpoch)) {
        return false;
      }
      notification.success(t('messages.deleteSuccess', { name: skill.name }));
      await loadSkills(true);
      return capabilityIsCurrent(capabilityEpoch);
    } catch (err) {
      if (!capabilityIsCurrent(capabilityEpoch)) {
        return false;
      }
      notification.error(
        t('messages.deleteFailed', {
          error: err instanceof Error ? err.message : String(err),
        }),
      );
      return false;
    }
  }, [capabilityIsCurrent, currentCapabilityEpoch, loadSkills, notification, t, workspacePath]);

  const handleGlobalSkillToggle = useCallback(async (skill: SkillInfo, enabled: boolean) => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null || skill.level !== 'user') {
      return false;
    }

    setSavingGlobalSkillKey(skill.key);
    try {
      const settings = await configAPI.setGlobalSkillDisabled({
        skillKey: skill.key,
        disabled: !enabled,
      });
      if (!capabilityIsCurrent(capabilityEpoch)) {
        return false;
      }

      setGloballyDisabledSkillKeys(new Set(settings.globallyDisabledUserSkillKeys));
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      notification.success(t('messages.toggleSuccess', {
        name: skill.name,
        status: enabled ? t('messages.enabled') : t('messages.disabled'),
      }));
      return true;
    } catch (err) {
      if (!capabilityIsCurrent(capabilityEpoch)) {
        return false;
      }
      log.error('Failed to update global Skill availability', {
        skillKey: skill.key,
        enabled,
        error: err,
      });
      notification.error(t('messages.toggleFailed', {
        error: err instanceof Error ? err.message : String(err),
      }));
      return false;
    } finally {
      if (capabilityIsCurrent(capabilityEpoch)) {
        setSavingGlobalSkillKey(null);
      }
    }
  }, [capabilityIsCurrent, currentCapabilityEpoch, notification, t]);

  const normalizedQuery = searchQuery.trim().toLowerCase();

  const filteredSkills = useMemo(() => {
    return skills.filter((skill) => {
      let matchesFilter = true;
      if (activeFilter !== 'all') {
        matchesFilter = installedSkillGroup(skill) === activeFilter;
      }

      const matchesQuery = !normalizedQuery || [
        skill.name,
        skill.description,
        skill.path,
      ].some((field) => field?.toLowerCase().includes(normalizedQuery));
      return matchesFilter && matchesQuery;
    });
  }, [activeFilter, normalizedQuery, skills]);

  const { counts, sourceGroups } = useMemo(() => {
    const counts: Record<InstalledFilter, number> = {
      all: skills.length, builtin: 0, user: 0, project: 0,
    };
    const sources = new Map<`source:${string}`, string>();
    for (const skill of skills) {
      const group = installedSkillGroup(skill);
      counts[group] = (counts[group] ?? 0) + 1;
      if (group.startsWith('source:')) {
        sources.set(group as `source:${string}`, getSkillSourceLabel(skill, t('list.item.unknownSource')));
      }
    }
    return {
      counts,
      sourceGroups: [...sources].sort(([left], [right]) => left.localeCompare(right))
        .map(([id, label]) => ({ id, label })),
    };
  }, [skills, t]);

  return {
    catalogContextKey: capabilityKey,
    catalogReady: enabled && loadedContextKey === capabilityKey && !loading && !error,
    skills,
    diagnostics,
    diagnosticsAvailable,
    globallyDisabledSkillKeys,
    savingGlobalSkillKey,
    filteredSkills,
    counts,
    sourceGroups,
    loading,
    error,
    loadSkills,
    handleDelete,
    handleGlobalSkillToggle,
    formLevel,
    setFormLevel,
    formPath,
    setFormPath,
    validationResult,
    isValidating,
    isAdding,
    handleBrowse,
    handleAdd,
    resetForm,
    workspacePath,
    hasWorkspace,
    isRemoteWorkspace,
  };
}
