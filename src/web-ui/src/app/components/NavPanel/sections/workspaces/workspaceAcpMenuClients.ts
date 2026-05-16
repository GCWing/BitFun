import {
  ACPClientAPI,
  type AcpClientInfo,
} from '@/infrastructure/api/service-api/ACPClientAPI';

const REMOTE_ACP_PRESETS = [
  { id: 'opencode', name: 'opencode' },
  { id: 'claude-code', name: 'Claude Code' },
  { id: 'codex', name: 'Codex' },
] as const;

interface LoadWorkspaceAcpMenuClientsOptions {
  remoteWorkspace?: boolean;
  remoteConnectionId?: string;
}

function virtualRemoteClient(id: string, name: string): AcpClientInfo {
  return {
    id,
    name,
    command: '',
    args: [],
    enabled: true,
    readonly: false,
    permissionMode: 'ask',
    status: 'configured',
    toolName: `acp__${id}__prompt`,
    sessionCount: 0,
  };
}

export async function loadWorkspaceAcpMenuClients(
  options: LoadWorkspaceAcpMenuClientsOptions = {}
): Promise<AcpClientInfo[]> {
  const clients = await ACPClientAPI.getClients();

  if (!options.remoteWorkspace) {
    return clients.filter(client => client.enabled);
  }

  if (!options.remoteConnectionId) {
    return [];
  }

  const probes = await ACPClientAPI.probeClientRequirements({
    remoteConnectionId: options.remoteConnectionId,
  });
  const runnableRemoteIds = new Set(
    probes.filter(probe => probe.runnable).map(probe => probe.id)
  );
  const clientsById = new Map(clients.map(client => [client.id, client]));
  return REMOTE_ACP_PRESETS
    .filter(({ id }) => runnableRemoteIds.has(id))
    .map(({ id, name }) => {
    const configured = clientsById.get(id);
    if (!configured) {
      return virtualRemoteClient(id, name);
    }
    return {
      ...configured,
      name: configured.name || name,
      enabled: true,
    };
    });
}
