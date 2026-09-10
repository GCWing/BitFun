import { canonicalAgentId } from '../../../shared/agent-harness/contract.generated';
const STORAGE_KEY = 'openbitfun.mobile.navigation.v1';

export interface MobileNavigationScope {
  accountId: string;
  controllerDeviceId: string;
  relayUrl: string;
  routeKey: string;
}

export interface MobileNavigation {
  deviceId: string;
  session?: { id: string; name: string; agentType: string };
}

export interface PairedNavigation {
  scope: MobileNavigationScope;
  restored: MobileNavigation | null;
}

type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

function browserStorage(): StorageLike | null {
  try { return typeof window === 'undefined' ? null : window.sessionStorage; }
  catch { return null; }
}

export function loadMobileNavigation(
  scope: MobileNavigationScope,
  storage: StorageLike | null = browserStorage(),
): MobileNavigation | null {
  try {
    const raw = storage?.getItem(STORAGE_KEY);
    if (!raw) return null;
    const record = JSON.parse(raw);
    if (record.version !== 1
      || record.accountId !== scope.accountId
      || record.controllerDeviceId !== scope.controllerDeviceId
      || record.relayUrl !== scope.relayUrl
      || record.routeKey !== scope.routeKey
      || typeof record.deviceId !== 'string'
      || !record.deviceId.trim()) return null;
    const session = record.session;
    return {
      deviceId: record.deviceId,
      session: session && typeof session.id === 'string' && session.id.trim()
        && typeof session.name === 'string' && typeof session.agentType === 'string'
        ? { id: session.id, name: session.name, agentType: canonicalAgentId(session.agentType) }
        : undefined,
    };
  } catch {
    // Keep unreadable/newer records intact; live navigation remains usable.
    return null;
  }
}

export function saveMobileNavigation(
  scope: MobileNavigationScope,
  navigation: MobileNavigation,
  storage: StorageLike | null = browserStorage(),
): void {
  try { storage?.setItem(STORAGE_KEY, JSON.stringify({ version: 1, ...scope, ...navigation })); }
  catch { /* Browser storage is optional. */ }
}

export function clearMobileNavigation(storage: StorageLike | null = browserStorage()): void {
  try { storage?.removeItem(STORAGE_KEY); }
  catch { /* Browser storage is optional. */ }
}
