import type { MiniAppMeta, MiniAppPermissions } from '@/infrastructure/api/service-api/MiniAppAPI';

export type MiniAppDetailSource = 'builtin' | 'market' | 'installed';

export type MiniAppDetailCapabilityKind =
  | 'ai'
  | 'workspace'
  | 'export'
  | 'shell'
  | 'network'
  | 'worker'
  | 'storage'
  | 'desktop'
  | 'surface'
  | 'controlled'
  | 'instant';

export interface MiniAppDetailCapability {
  kind: MiniAppDetailCapabilityKind;
  count?: number;
  value?: string;
}

const APP_DATA_SCOPE = '{appdata}';
const WORKSPACE_SCOPE = '{workspace}';

function includesScope(permissions: MiniAppPermissions, scope: string): boolean {
  return [
    ...(permissions.fs?.read ?? []),
    ...(permissions.fs?.write ?? []),
  ].includes(scope);
}

function countDesktopCapabilities(permissions: MiniAppPermissions): number {
  const host = permissions.host;
  if (!host) return 0;

  return [
    host.dialog,
    host.clipboard_read,
    host.clipboard_write,
    host.open_external,
    host.reveal_in_folder,
    host.chat_composer,
    host.system_info,
    permissions.notifications?.system,
  ].filter(Boolean).length;
}

/**
 * Projects the installed manifest into three user-facing capability facts.
 *
 * The detail card intentionally does not invent app-specific marketing copy:
 * concrete manifest capabilities win, and generic Mini App product truths fill
 * any remaining slots so legacy or minimal manifests still have a complete
 * layout.
 */
export function projectMiniAppDetailCapabilities(
  app: Pick<MiniAppMeta, 'permissions'>,
): MiniAppDetailCapability[] {
  const permissions = app.permissions ?? {};
  const capabilities: MiniAppDetailCapability[] = [];

  if (permissions.ai?.enabled || permissions.agent?.enabled) {
    capabilities.push({ kind: 'ai' });
  }
  if (includesScope(permissions, WORKSPACE_SCOPE)) {
    capabilities.push({ kind: 'workspace' });
  }
  if (permissions.host?.deck_render) {
    capabilities.push({ kind: 'export' });
  }
  if (permissions.shell?.allow?.length) {
    capabilities.push({
      kind: 'shell',
      value: permissions.shell.allow.slice(0, 3).join(', '),
    });
  }
  if (permissions.net?.allow?.length) {
    capabilities.push({ kind: 'network', count: permissions.net.allow.length });
  }
  if (permissions.node?.enabled) {
    capabilities.push({ kind: 'worker' });
  }
  if (includesScope(permissions, APP_DATA_SCOPE)) {
    capabilities.push({ kind: 'storage' });
  }

  const desktopCapabilityCount = countDesktopCapabilities(permissions);
  if (desktopCapabilityCount > 0) {
    capabilities.push({ kind: 'desktop', count: desktopCapabilityCount });
  }

  const fallbackCapabilities: MiniAppDetailCapability[] = [
    { kind: 'surface' },
    { kind: 'controlled' },
    { kind: 'instant' },
  ];

  for (const capability of fallbackCapabilities) {
    if (capabilities.length >= 3) break;
    capabilities.push(capability);
  }

  return capabilities.slice(0, 3);
}

export function resolveMiniAppDetailSource(
  appId: string,
  hasMarketOrigin: boolean,
): MiniAppDetailSource {
  if (appId.startsWith('builtin-')) return 'builtin';
  if (hasMarketOrigin) return 'market';
  return 'installed';
}
