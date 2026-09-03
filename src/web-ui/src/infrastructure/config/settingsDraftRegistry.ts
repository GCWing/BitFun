import { useLayoutEffect, useRef, useSyncExternalStore } from 'react';

export interface SettingsDraftScope {
  pageId: string;
  viewId?: string;
}

export interface SettingsDraftRegistration extends SettingsDraftScope {
  id: string;
  label: string;
  dirty: boolean;
  saving?: boolean;
  save: () => boolean | void | Promise<boolean | void>;
  discard: () => void | Promise<void>;
}

export interface SettingsDraftNavigationTarget extends SettingsDraftScope {
  kind: 'settings';
}

interface RegisteredSettingsDraft extends SettingsDraftRegistration {
  token: symbol;
}

export interface PendingSettingsNavigation {
  id: number;
  resourceIds: readonly string[];
  resourceLabels: readonly string[];
  action: 'save' | 'discard' | null;
  failed: boolean;
}

export interface SettingsDraftSnapshot {
  revision: number;
  resources: readonly SettingsDraftRegistration[];
  pendingNavigation: PendingSettingsNavigation | null;
}

interface InternalPendingNavigation extends PendingSettingsNavigation {
  commit: () => void;
}

const resources = new Map<string, RegisteredSettingsDraft>();
const listeners = new Set<() => void>();
let revision = 0;
let pendingSequence = 0;
let pendingNavigation: InternalPendingNavigation | null = null;
let snapshot: SettingsDraftSnapshot = {
  revision,
  resources: [],
  pendingNavigation: null,
};

function publicRegistration(resource: RegisteredSettingsDraft): SettingsDraftRegistration {
  const { token: _token, ...registration } = resource;
  return registration;
}

function publish(): void {
  revision += 1;
  snapshot = {
    revision,
    resources: Array.from(resources.values(), publicRegistration),
    pendingNavigation: pendingNavigation ? {
      id: pendingNavigation.id,
      resourceIds: pendingNavigation.resourceIds,
      resourceLabels: pendingNavigation.resourceLabels,
      action: pendingNavigation.action,
      failed: pendingNavigation.failed,
    } : null,
  };
  listeners.forEach(listener => listener());
}

function isResourceActiveAt(
  resource: SettingsDraftRegistration,
  destination: SettingsDraftScope,
): boolean {
  return resource.pageId === destination.pageId
    && (resource.viewId === undefined || resource.viewId === destination.viewId);
}

function isResourceLeftBy(
  resource: SettingsDraftRegistration,
  target: SettingsDraftNavigationTarget | null,
): boolean {
  if (!target) return true;
  if (resource.pageId !== target.pageId) return true;
  return resource.viewId !== undefined && resource.viewId !== target.viewId;
}

function blockingResources(
  from: SettingsDraftScope,
  target: SettingsDraftNavigationTarget | null,
): RegisteredSettingsDraft[] {
  return Array.from(resources.values()).filter(resource => (
    resource.dirty
    && isResourceActiveAt(resource, from)
    && isResourceLeftBy(resource, target)
  ));
}

function setPendingAction(
  expectedId: number,
  action: PendingSettingsNavigation['action'],
  failed = false,
): boolean {
  if (!pendingNavigation || pendingNavigation.id !== expectedId) return false;
  pendingNavigation = { ...pendingNavigation, action, failed };
  publish();
  return true;
}

function markPendingNavigationFailed(expectedId: number): boolean {
  if (!pendingNavigation || pendingNavigation.id !== expectedId) return false;
  const remainingResources = pendingNavigation.resourceIds
    .map(resourceId => resources.get(resourceId))
    .filter((resource): resource is RegisteredSettingsDraft => Boolean(resource?.dirty));
  pendingNavigation = {
    ...pendingNavigation,
    resourceIds: remainingResources.length > 0
      ? remainingResources.map(resource => resource.id)
      : pendingNavigation.resourceIds,
    resourceLabels: remainingResources.length > 0
      ? remainingResources.map(resource => resource.label)
      : pendingNavigation.resourceLabels,
    action: null,
    failed: true,
  };
  publish();
  return true;
}

function finishPendingNavigation(expectedId: number): boolean {
  if (!pendingNavigation || pendingNavigation.id !== expectedId) return false;
  const commit = pendingNavigation.commit;
  pendingNavigation = null;
  publish();
  commit();
  return true;
}

function waitForResourceIdle(resourceId: string): Promise<void> {
  if (!resources.get(resourceId)?.saving) return Promise.resolve();
  return new Promise(resolve => {
    const unsubscribe = subscribeSettingsDrafts(() => {
      if (resources.get(resourceId)?.saving) return;
      unsubscribe();
      resolve();
    });
  });
}

export function registerSettingsDraft(registration: SettingsDraftRegistration): () => void {
  const token = Symbol(registration.id);
  resources.set(registration.id, { ...registration, token });
  publish();
  return () => {
    if (resources.get(registration.id)?.token !== token) return;
    resources.delete(registration.id);
    publish();
  };
}

export function updateSettingsDraft(
  id: string,
  update: Omit<Partial<SettingsDraftRegistration>, 'id'>,
): void {
  const current = resources.get(id);
  if (!current) return;
  const next = { ...current, ...update };
  resources.set(id, next);
  if (
    current.pageId !== next.pageId
    || current.viewId !== next.viewId
    || current.label !== next.label
    || current.dirty !== next.dirty
    || current.saving !== next.saving
  ) {
    publish();
  }
}

/**
 * Runs a navigation immediately when no active draft is being left. Otherwise
 * SettingsScene presents the single shared save/discard decision.
 */
export function requestSettingsNavigation(
  from: SettingsDraftScope,
  target: SettingsDraftNavigationTarget | null,
  commit: () => void,
): boolean {
  const blockers = blockingResources(from, target);
  if (blockers.length === 0) {
    commit();
    return true;
  }
  if (pendingNavigation) return false;

  pendingNavigation = {
    id: ++pendingSequence,
    resourceIds: blockers.map(resource => resource.id),
    resourceLabels: blockers.map(resource => resource.label),
    action: null,
    failed: false,
    commit,
  };
  publish();
  return false;
}

/** Guard closing a page-owned editor or dialog through the same shared flow. */
export function requestSettingsDraftExit(
  resourceIds: readonly string[],
  commit: () => void,
): boolean {
  const blockers = resourceIds
    .map(id => resources.get(id))
    .filter((resource): resource is RegisteredSettingsDraft => Boolean(resource?.dirty));
  if (blockers.length === 0) {
    commit();
    return true;
  }
  if (pendingNavigation) return false;

  pendingNavigation = {
    id: ++pendingSequence,
    resourceIds: blockers.map(resource => resource.id),
    resourceLabels: blockers.map(resource => resource.label),
    action: null,
    failed: false,
    commit,
  };
  publish();
  return false;
}

export function requestAllSettingsDraftsExit(commit: () => void): boolean {
  return requestSettingsDraftExit(Array.from(resources.keys()), commit);
}

export function cancelPendingSettingsNavigation(): void {
  if (!pendingNavigation || pendingNavigation.action) return;
  pendingNavigation = null;
  publish();
}

export async function saveAndContinueSettingsNavigation(): Promise<boolean> {
  const pending = pendingNavigation;
  if (!pending || pending.action) return false;
  if (!setPendingAction(pending.id, 'save')) return false;

  try {
    for (const resourceId of pending.resourceIds) {
      await waitForResourceIdle(resourceId);
      const resource = resources.get(resourceId);
      if (!resource?.dirty) continue;
      const saved = await resource.save();
      if (saved === false) throw new Error('settings draft save rejected');
    }
    return finishPendingNavigation(pending.id);
  } catch {
    markPendingNavigationFailed(pending.id);
    return false;
  }
}

export async function discardAndContinueSettingsNavigation(): Promise<boolean> {
  const pending = pendingNavigation;
  if (!pending || pending.action) return false;
  if (!setPendingAction(pending.id, 'discard')) return false;

  try {
    for (const resourceId of pending.resourceIds) {
      await waitForResourceIdle(resourceId);
      const resource = resources.get(resourceId);
      if (!resource?.dirty) continue;
      await resource.discard();
    }
    return finishPendingNavigation(pending.id);
  } catch {
    setPendingAction(pending.id, null, true);
    return false;
  }
}

export function subscribeSettingsDrafts(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getSettingsDraftSnapshot(): SettingsDraftSnapshot {
  return snapshot;
}

export function useSettingsDraftSnapshot(): SettingsDraftSnapshot {
  return useSyncExternalStore(
    subscribeSettingsDrafts,
    getSettingsDraftSnapshot,
    getSettingsDraftSnapshot,
  );
}

export function useSettingsDraft(
  registration: SettingsDraftRegistration & { enabled?: boolean },
): void {
  const registrationRef = useRef(registration);
  registrationRef.current = registration;
  const callbacksRef = useRef({
    save: registration.save,
    discard: registration.discard,
  });
  callbacksRef.current = {
    save: registration.save,
    discard: registration.discard,
  };

  useLayoutEffect(() => {
    const currentRegistration = registrationRef.current;
    if (currentRegistration.enabled === false) return undefined;
    const { enabled: _enabled, ...draft } = currentRegistration;
    return registerSettingsDraft({
      ...draft,
      save: () => callbacksRef.current.save(),
      discard: () => callbacksRef.current.discard(),
    });
  }, [registration.enabled, registration.id, registration.pageId, registration.viewId]);

  useLayoutEffect(() => {
    if (registration.enabled === false) return;
    updateSettingsDraft(registration.id, {
      label: registration.label,
      dirty: registration.dirty,
      saving: registration.saving,
    });
  }, [
    registration.dirty,
    registration.enabled,
    registration.id,
    registration.label,
    registration.saving,
  ]);
}

/**
 * A device-surface switch invalidates the settings owner itself. Abandon the
 * in-memory registry without invoking save/discard callbacks against a host
 * whose transport is being replaced.
 */
export function abandonSettingsDraftsForContextSwitch(): void {
  if (resources.size === 0 && pendingNavigation === null) return;
  resources.clear();
  pendingNavigation = null;
  publish();
}

if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', event => {
    if (!Array.from(resources.values()).some(resource => resource.dirty)) return;
    event.preventDefault();
    event.returnValue = '';
  });
}

/** Test-only reset for the module-level registry. */
export function resetSettingsDraftRegistryForTests(): void {
  resources.clear();
  pendingNavigation = null;
  pendingSequence = 0;
  publish();
}
