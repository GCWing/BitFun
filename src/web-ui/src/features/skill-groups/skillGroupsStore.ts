import { createStore } from 'zustand/vanilla';
import type { UserSkillGroup, UserSkillGroupsConfig } from '@/infrastructure/config/types';
import type { SurfaceScope } from '@/infrastructure/peer-device/deviceSurface';
import { createUserSkillGroupsConfig, normalizeUserSkillGroupsConfig, SkillGroupError } from './skillGroups';

interface SkillGroupsRepository {
  read: () => Promise<unknown>;
  write: (config: UserSkillGroupsConfig) => Promise<void>;
}

interface SkillGroupsState {
  epoch: number;
  groups: UserSkillGroup[];
  status: 'idle' | 'loading' | 'ready' | 'error';
  error: unknown;
  saving: boolean;
  load: (scope: SurfaceScope, force?: boolean) => Promise<void>;
  update: (scope: SurfaceScope, change: (groups: UserSkillGroup[]) => UserSkillGroup[]) => Promise<void>;
}

/** One collection per active host, shared by management and agent pickers. */
export function createSkillGroupsStore(repository: SkillGroupsRepository) {
  let requestId = 0;
  let pendingLoad: Promise<void> | undefined;
  return createStore<SkillGroupsState>((set, get) => ({
    epoch: 0, groups: [], status: 'idle', error: null, saving: false,
    load: (scope, force = false) => {
      if (!scope.isCurrent()) return Promise.resolve();
      const current = get();
      if (current.epoch === scope.epoch) {
        if (current.saving || (!force && current.status === 'ready')) return Promise.resolve();
        if (current.status === 'loading' && pendingLoad) return pendingLoad;
      }
      const request = ++requestId;
      set({
        epoch: scope.epoch, groups: current.epoch === scope.epoch ? current.groups : [],
        status: 'loading', error: null, saving: false,
      });
      pendingLoad = (async () => {
        try {
          const config = normalizeUserSkillGroupsConfig(await repository.read());
          if (scope.isCurrent() && request === requestId) {
            set({ groups: config.groups, status: 'ready', error: null });
          }
        } catch (error) {
          if (scope.isCurrent() && request === requestId) set({ status: 'error', error });
        }
      })();
      return pendingLoad;
    },
    update: async (scope, change) => {
      scope.assertCurrent('update skill groups');
      const state = get();
      if (state.epoch !== scope.epoch || state.status !== 'ready' || state.saving) {
        throw new SkillGroupError('notReady');
      }
      set({ saving: true });
      try {
        // Re-read before a mutation so a stale page does not overwrite unrelated edits.
        const latest = normalizeUserSkillGroupsConfig(await repository.read());
        scope.assertCurrent('update skill groups');
        const next = createUserSkillGroupsConfig(change(latest.groups));
        await repository.write(next);
        scope.assertCurrent('update skill groups');
        set({ groups: next.groups, error: null });
      } finally {
        if (scope.isCurrent()) set({ saving: false });
      }
    },
  }));
}
