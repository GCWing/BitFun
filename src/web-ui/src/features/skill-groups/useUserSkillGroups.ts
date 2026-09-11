import { useCallback, useEffect, useSyncExternalStore } from 'react';
import { useStore } from 'zustand';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import type { UserSkillGroup } from '@/infrastructure/config/types';
import { getActiveSurfaceScope, onSurfaceActivated } from '@/infrastructure/peer-device/deviceSurface';
import { USER_SKILL_GROUPS_CONFIG_PATH } from './skillGroups';
import { createSkillGroupsStore } from './skillGroupsStore';

const store = createSkillGroupsStore({
  read: () => configAPI.getConfig(USER_SKILL_GROUPS_CONFIG_PATH),
  write: config => configAPI.setConfig(USER_SKILL_GROUPS_CONFIG_PATH, config),
});
const EMPTY_GROUPS: UserSkillGroup[] = [];

export function useUserSkillGroups(enabled = true) {
  const scope = useSyncExternalStore(onSurfaceActivated, getActiveSurfaceScope, getActiveSurfaceScope);
  const state = useStore(store);
  const { load, update } = state;
  useEffect(() => {
    if (enabled) void load(scope);
  }, [enabled, load, scope]);
  const reload = useCallback(() => enabled ? load(scope, true) : Promise.resolve(), [enabled, load, scope]);
  const updateGroups = useCallback((change: (groups: UserSkillGroup[]) => UserSkillGroup[]) => {
    if (!enabled) return Promise.reject(new Error('Skill group management is unavailable'));
    return update(scope, change);
  }, [enabled, scope, update]);
  const current = enabled && state.epoch === scope.epoch;
  return {
    scope,
    groups: current ? state.groups : EMPTY_GROUPS,
    loading: enabled && (!current || state.status === 'idle' || state.status === 'loading'),
    error: current ? state.error : null,
    ready: current && state.status === 'ready',
    saving: current && state.saving,
    reload, updateGroups,
  };
}
