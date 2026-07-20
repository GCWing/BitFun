import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { configAPI } from '@/infrastructure/api';
import type { SkillInfo, SkillLevel, SkillValidationResult } from '@/infrastructure/config/types';
import { canDeleteSkill } from '@/infrastructure/config/skillSourcePresentation';
import { useWorkspaceManagerSync } from '@/infrastructure/hooks/useWorkspaceManagerSync';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type { InstalledFilter } from '../skillsSceneStore';

const log = createLogger('SkillsScene:useInstalledSkills');

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
  const { workspacePath, hasWorkspace, isRemoteWorkspace } = useWorkspaceManagerSync();

  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [formLevel, setFormLevel] = useState<SkillLevel>('user');
  const [formPath, setFormPath] = useState('');
  const [validationResult, setValidationResult] = useState<SkillValidationResult | null>(null);
  const [isValidating, setIsValidating] = useState(false);
  const [isAdding, setIsAdding] = useState(false);
  const loadRequestIdRef = useRef(0);
  const validationRequestIdRef = useRef(0);
  const capabilityKey = `${enabled}\u0000${workspacePath ?? ''}\u0000${isRemoteWorkspace}`;
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
      const list = await configAPI.getSkillConfigs({
        forceRefresh,
        workspacePath: workspacePath || undefined,
      });
      if (requestId !== loadRequestIdRef.current || !capabilityIsCurrent(capabilityEpoch)) {
        return;
      }
      setSkills(list);
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
  }, [capabilityIsCurrent, currentCapabilityEpoch, workspacePath]);

  useEffect(() => {
    loadRequestIdRef.current += 1;
    validationRequestIdRef.current += 1;
    setValidationResult(null);
    setIsValidating(false);
    setIsAdding(false);
    if (!enabled) {
      setSkills([]);
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

  const normalizedQuery = searchQuery.trim().toLowerCase();

  const filteredSkills = useMemo(() => {
    return skills.filter((skill) => {
      let matchesFilter = true;
      if (activeFilter === 'user') {
        matchesFilter = skill.level === 'user' && !skill.isBuiltin;
      } else if (activeFilter === 'project') {
        matchesFilter = skill.level === 'project' && !skill.isBuiltin;
      } else if (activeFilter === 'builtin') {
        matchesFilter = skill.isBuiltin;
      } else if (activeFilter === 'suite') {
        matchesFilter = skill.isBuiltin;
      }

      const matchesQuery = !normalizedQuery || [
        skill.name,
        skill.description,
        skill.path,
      ].some((field) => field?.toLowerCase().includes(normalizedQuery));
      return matchesFilter && matchesQuery;
    });
  }, [activeFilter, normalizedQuery, skills]);

  const counts = useMemo(() => ({
    all: skills.length,
    builtin: skills.filter((skill) => skill.isBuiltin).length,
    user: skills.filter((skill) => skill.level === 'user' && !skill.isBuiltin).length,
    project: skills.filter((skill) => skill.level === 'project' && !skill.isBuiltin).length,
    suite: skills.filter((skill) => skill.isBuiltin).length,
  }), [skills]);

  return {
    skills,
    filteredSkills,
    counts,
    loading,
    error,
    loadSkills,
    handleDelete,
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
