import {
  ACPClientAPI,
  type AcpClientInfo,
} from '@/infrastructure/api/service-api/ACPClientAPI';

interface LoadWorkspaceAcpMenuClientsOptions {
  remoteWorkspace?: boolean;
  remoteConnectionId?: string;
}

const forcedRemoteRequirementRefreshes = new Set<string>();

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

  const enabledClients = clients.filter(client => client.enabled);
  if (enabledClients.length === 0) {
    return [];
  }

  let probes = await ACPClientAPI.probeClientRequirements({
    remoteConnectionId: options.remoteConnectionId,
  });
  if (probes.length === 0) {
    probes = await ACPClientAPI.probeClientRequirements({
      remoteConnectionId: options.remoteConnectionId,
      force: true,
    });
  }
  let visibleClients = filterRunnableClients(enabledClients, probes);
  if (
    visibleClients.length === 0 &&
    !forcedRemoteRequirementRefreshes.has(options.remoteConnectionId)
  ) {
    forcedRemoteRequirementRefreshes.add(options.remoteConnectionId);
    probes = await ACPClientAPI.probeClientRequirements({
      remoteConnectionId: options.remoteConnectionId,
      force: true,
    });
    visibleClients = filterRunnableClients(enabledClients, probes);
  }
  return visibleClients;
}

function filterRunnableClients(
  clients: AcpClientInfo[],
  probes: Awaited<ReturnType<typeof ACPClientAPI.probeClientRequirements>>
): AcpClientInfo[] {
  const runnableRemoteIds = new Set(
    probes.filter(probe => probe.runnable).map(probe => probe.id)
  );
  return clients.filter(client => runnableRemoteIds.has(client.id));
}
