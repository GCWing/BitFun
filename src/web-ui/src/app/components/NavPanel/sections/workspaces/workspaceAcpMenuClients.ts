import {
  ACPClientAPI,
  type AcpClientInfo,
} from '@/infrastructure/api/service-api/ACPClientAPI';

export async function loadWorkspaceAcpMenuClients(): Promise<AcpClientInfo[]> {
  const clients = await ACPClientAPI.getClients();
  return clients.filter(client => client.enabled);
}
