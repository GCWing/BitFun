import { beforeEach, describe, expect, it } from 'vitest';
import type { UserSkillGroupsConfig } from '@/infrastructure/config/types';
import { activateSurface, getActiveSurfaceScope, resetDeviceSurfaceForTest } from '@/infrastructure/peer-device/deviceSurface';
import { saveUserSkillGroup } from './skillGroups';
import { createSkillGroupsStore } from './skillGroupsStore';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(resolvePromise => { resolve = resolvePromise; });
  return { promise, resolve };
}

const initial: UserSkillGroupsConfig = {
  version: 1, groups: [{ id: 'daily', name: 'Daily', skillKeys: ['unavailable::skill'] }],
};

describe('shared skill group persistence', () => {
  beforeEach(() => resetDeviceSurfaceForTest());

  it('shares one load and merges edits with fresh persisted data', async () => {
    let saved = structuredClone(initial);
    let reads = 0;
    let writes = 0;
    const store = createSkillGroupsStore({
      read: async () => { reads += 1; return saved; },
      write: async config => { writes += 1; saved = config; },
    });
    const scope = getActiveSurfaceScope();
    await Promise.all([store.getState().load(scope), store.getState().load(scope)]);
    expect(reads).toBe(1);
    const original = store.getState().groups[0];
    saved = { ...saved, groups: [...saved.groups, { id: 'new', name: 'New elsewhere', skillKeys: [] }] };
    await store.getState().update(scope, groups => saveUserSkillGroup(groups, { ...original, name: 'Renamed' }, original));
    expect(writes).toBe(1);
    expect(saved.groups).toEqual([{ ...original, name: 'Renamed' }, { id: 'new', name: 'New elsewhere', skillKeys: [] }]);
    expect(store.getState().groups).toEqual(saved.groups);
  });

  it('does not permit a write after a failed load', async () => {
    let writes = 0;
    const store = createSkillGroupsStore({
      read: async () => { throw new Error('Host unavailable'); },
      write: async () => { writes += 1; },
    });
    const scope = getActiveSurfaceScope();
    await store.getState().load(scope);
    expect(store.getState().status).toBe('error');
    await expect(store.getState().update(scope, () => [])).rejects.toThrow();
    expect(writes).toBe(0);
  });

  it('refuses to overwrite data that changed to a newer format after loading', async () => {
    let saved: unknown = initial;
    let writes = 0;
    const store = createSkillGroupsStore({ read: async () => saved, write: async () => { writes += 1; } });
    const scope = getActiveSurfaceScope();
    await store.getState().load(scope);
    saved = { version: 2, groups: [] };
    await expect(store.getState().update(scope, () => [])).rejects.toThrow();
    expect(writes).toBe(0);
    expect(store.getState().groups).toEqual(initial.groups);
    expect(store.getState().saving).toBe(false);
  });

  it('keeps committed groups after a failed save and allows retry', async () => {
    let fail = true;
    const store = createSkillGroupsStore({
      read: async () => initial,
      write: async () => { if (fail) throw new Error('Write failed'); },
    });
    const scope = getActiveSurfaceScope();
    await store.getState().load(scope);
    await expect(store.getState().update(scope, () => [])).rejects.toThrow('Write failed');
    expect(store.getState().groups).toEqual(initial.groups);
    expect(store.getState().saving).toBe(false);
    fail = false;
    await store.getState().update(scope, () => []);
    expect(store.getState().groups).toEqual([]);
  });

  it('rejects a second mutation while a save is pending', async () => {
    const pending = deferred<void>();
    const store = createSkillGroupsStore({ read: async () => initial, write: () => pending.promise });
    const scope = getActiveSurfaceScope();
    await store.getState().load(scope);
    const saving = store.getState().update(scope, groups => groups);
    await expect(store.getState().update(scope, () => [])).rejects.toThrow();
    pending.resolve();
    await saving;
    expect(store.getState().groups).toEqual(initial.groups);
  });

  it('drops responses from a previous device activation', async () => {
    const oldRead = deferred<unknown>();
    let reads = 0;
    const store = createSkillGroupsStore({
      read: () => ++reads === 1 ? oldRead.promise : Promise.resolve({ version: 1, groups: [] }),
      write: async () => {},
    });
    const oldLoad = store.getState().load(getActiveSurfaceScope());
    const next = activateSurface('peer-device');
    await store.getState().load(next);
    oldRead.resolve(initial);
    await oldLoad;
    expect(store.getState().epoch).toBe(next.epoch);
    expect(store.getState().groups).toEqual([]);
  });

  it('does not submit an old draft through another device transport', async () => {
    const readBeforeWrite = deferred<unknown>();
    let reads = 0;
    let writes = 0;
    const store = createSkillGroupsStore({
      read: () => ++reads === 2 ? readBeforeWrite.promise : Promise.resolve(initial),
      write: async () => { writes += 1; },
    });
    const scope = getActiveSurfaceScope();
    await store.getState().load(scope);
    const saving = store.getState().update(scope, () => []);
    const rejected = expect(saving).rejects.toMatchObject({ isSurfaceChangedError: true });
    activateSurface('peer-device');
    readBeforeWrite.resolve(initial);
    await rejected;
    expect(writes).toBe(0);
  });
});
