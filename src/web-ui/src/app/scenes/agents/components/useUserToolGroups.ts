import { useCallback, useEffect, useState } from 'react';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import type { UserToolGroup } from '@/infrastructure/config/types';
import { globalEventBus } from '@/infrastructure/event-bus';
import {
  USER_TOOL_GROUPS_CONFIG_PATH,
  createUserToolGroupsConfig,
  normalizeUserToolGroupsConfig,
} from './toolGroups';

const USER_TOOL_GROUPS_UPDATED_EVENT = 'user-tool-groups:updated';

export function useUserToolGroups() {
  const [groups, setGroups] = useState<UserToolGroup[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    void configAPI.getConfig(USER_TOOL_GROUPS_CONFIG_PATH)
      .then((value) => {
        if (active) {
          setGroups(normalizeUserToolGroupsConfig(value).groups);
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
        setGroups(normalizeUserToolGroupsConfig(value).groups);
      }
    };
    globalEventBus.on(USER_TOOL_GROUPS_UPDATED_EVENT, handleUpdated);

    return () => {
      active = false;
      globalEventBus.off(USER_TOOL_GROUPS_UPDATED_EVENT, handleUpdated);
    };
  }, []);

  const saveGroups = useCallback(async (nextGroups: UserToolGroup[]) => {
    const next = createUserToolGroupsConfig(nextGroups);
    await configAPI.setConfig(USER_TOOL_GROUPS_CONFIG_PATH, next);
    setGroups(next.groups);
    globalEventBus.emit(USER_TOOL_GROUPS_UPDATED_EVENT, next);
  }, []);

  return { groups, loading, saveGroups };
}
