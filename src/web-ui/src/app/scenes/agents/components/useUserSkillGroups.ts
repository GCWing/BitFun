import { useCallback, useEffect, useState } from 'react';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import type { UserSkillGroup } from '@/infrastructure/config/types';
import { globalEventBus } from '@/infrastructure/event-bus';
import {
  USER_SKILL_GROUPS_CONFIG_PATH,
  createUserSkillGroupsConfig,
  normalizeUserSkillGroupsConfig,
} from './skillGroups';

const USER_SKILL_GROUPS_UPDATED_EVENT = 'user-skill-groups:updated';

export function useUserSkillGroups() {
  const [groups, setGroups] = useState<UserSkillGroup[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    void configAPI.getConfig(USER_SKILL_GROUPS_CONFIG_PATH)
      .then((value) => {
        if (active) {
          setGroups(normalizeUserSkillGroupsConfig(value).groups);
        }
      })
      .catch(() => {
        if (active) {
          setGroups([]);
        }
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    const handleUpdated = (value: unknown) => {
      if (active) {
        setGroups(normalizeUserSkillGroupsConfig(value).groups);
      }
    };
    globalEventBus.on(USER_SKILL_GROUPS_UPDATED_EVENT, handleUpdated);

    return () => {
      active = false;
      globalEventBus.off(USER_SKILL_GROUPS_UPDATED_EVENT, handleUpdated);
    };
  }, []);

  const saveGroups = useCallback(async (nextGroups: UserSkillGroup[]) => {
    const next = createUserSkillGroupsConfig(nextGroups);
    await configAPI.setConfig(USER_SKILL_GROUPS_CONFIG_PATH, next);
    setGroups(next.groups);
    globalEventBus.emit(USER_SKILL_GROUPS_UPDATED_EVENT, next);
  }, []);

  return { groups, loading, saveGroups };
}
