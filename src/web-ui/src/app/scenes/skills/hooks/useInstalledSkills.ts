import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
}

export function useInstalledSkills({ searchQuery, activeFilter }: UseInstalledSkillsOptions) {
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

  const loadSkills = useCallback(async (forceRefresh?: boolean) => {
    const requestId = ++loadRequestIdRef.current;

    try {
      setLoading(true);
      setError(null);
      const list = await configAPI.getSkillConfigs({
        forceRefresh,
        workspacePath: workspacePath || undefined,
      });
      if (requestId !== loadRequestIdRef.current) {
        return;
      }
      setSkills(list);
    } catch (err) {
      if (requestId !== loadRequestIdRef.current) {
        return;
      }
      log.error('Failed to load skills', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (requestId === loadRequestIdRef.current) {
        setLoading(false);
      }
    }
  }, [workspacePath]);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  const validatePath = useCallback(async (path: string) => {
    if (!path.trim()) {
      setValidationResult(null);
      return;
    }
    try {
      setIsValidating(true);
      const result = await configAPI.validateSkillPath(path);
      setValidationResult(result);
    } catch (err) {
      setValidationResult({
        valid: false,
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setIsValidating(false);
    }
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      validatePath(formPath);
    }, 300);
    return () => window.clearTimeout(timer);
  }, [formPath, validatePath]);

  const handleBrowse = useCallback(async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('form.path.label'),
      });
      if (selected) {
        setFormPath(selected as string);
      }
    } catch (err) {
      log.error('Failed to open file dialog', err);
    }
  }, [t]);

  const resetForm = useCallback(() => {
    setFormPath('');
    setFormLevel('user');
    setValidationResult(null);
  }, []);

  const handleAdd = useCallback(async () => {
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
      notification.success(t('messages.addSuccess', { name: validationResult.name }));
      resetForm();
      await loadSkills(true);
      return true;
    } catch (err) {
      notification.error(
        t('messages.addFailed', {
          error: err instanceof Error ? err.message : String(err),
        }),
      );
      return false;
    } finally {
      setIsAdding(false);
    }
  }, [formLevel, formPath, hasWorkspace, isRemoteWorkspace, loadSkills, notification, resetForm, t, validationResult, workspacePath]);

  const handleDelete = useCallback(async (skill: SkillInfo) => {
    if (!canDeleteSkill(skill)) {
      return false;
    }
    try {
      await configAPI.deleteSkill({
        skillKey: skill.key,
        workspacePath: workspacePath || undefined,
      });
      notification.success(t('messages.deleteSuccess', { name: skill.name }));
      await loadSkills(true);
      return true;
    } catch (err) {
      notification.error(
        t('messages.deleteFailed', {
          error: err instanceof Error ? err.message : String(err),
        }),
      );
      return false;
    }
  }, [loadSkills, notification, t, workspacePath]);

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
