import type { SavedConnection, SSHConfigEntry } from '../ssh-remote/types';

export interface RelayServerSearchState {
  filteredSavedConnections: SavedConnection[];
  filteredSSHConfigHosts: SSHConfigEntry[];
  hasSavedConnections: boolean;
  hasSSHConfigHosts: boolean;
}

export function getRelayConnectionHost(entry: SSHConfigEntry): string {
  return entry.hostname?.trim() || entry.host;
}

export function buildRelayServerSearchState(
  savedConnections: SavedConnection[],
  sshConfigHosts: SSHConfigEntry[],
  savedSearch: string,
  configSearch: string,
): RelayServerSearchState {
  const savedQuery = savedSearch.trim().toLowerCase();
  const configQuery = configSearch.trim().toLowerCase();

  return {
    filteredSavedConnections: savedConnections.filter((connection) => (
      !savedQuery
      || connection.name.toLowerCase().includes(savedQuery)
      || connection.host.toLowerCase().includes(savedQuery)
      || connection.username.toLowerCase().includes(savedQuery)
    )),
    filteredSSHConfigHosts: sshConfigHosts.filter((entry) => {
      if (!configQuery) return true;
      const hostname = getRelayConnectionHost(entry);
      return (
        entry.host.toLowerCase().includes(configQuery)
        || hostname.toLowerCase().includes(configQuery)
        || (entry.user || '').toLowerCase().includes(configQuery)
      );
    }),
    hasSavedConnections: savedConnections.length > 0,
    hasSSHConfigHosts: sshConfigHosts.length > 0,
  };
}
