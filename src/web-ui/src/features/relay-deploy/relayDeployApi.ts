/**
 * Relay Deploy Feature - API Service
 *
 * Wraps the desktop `relay_deploy_*` Tauri commands: deploy the open-source
 * BitFun relay server to a user-owned host over an existing SSH connection.
 */

import { api } from '@/infrastructure/api/service-api/ApiClient';

export type RelayDeployTask = 'install_docker' | 'deploy';
export type RelayTaskStatus = 'running' | 'succeeded' | 'failed';

export interface RelayPreflight {
  os: string;
  arch: string;
  archSupported: boolean;
  dockerInstalled: boolean;
  composeAvailable: boolean;
  /** "ok" | "sudo" | "unreachable" */
  dockerDaemon: string;
  curlAvailable: boolean;
  sudoAvailable: boolean;
  memTotalMb: number;
  portBusy: boolean;
  containerExists: boolean;
  relayHealthy: boolean;
  homeDir: string;
}

export interface RelayTaskPoll {
  cursor: number;
  output: string;
  status: RelayTaskStatus;
}

export interface RelayVerifyResult {
  reachable: boolean;
  version: string | null;
}

export const relayDeployApi = {
  /** Probe the remote environment (OS/arch, Docker, memory, port, existing relay). */
  async preflight(connectionId: string): Promise<RelayPreflight> {
    return api.invoke<RelayPreflight>('relay_deploy_preflight', { connectionId });
  },

  /** Start Docker installation on the remote host (detached; poll for progress). */
  async installDocker(connectionId: string): Promise<void> {
    return api.invoke('relay_deploy_install_docker', { connectionId });
  },

  /** Start the relay deployment on the remote host (detached; poll for progress). */
  async startDeploy(connectionId: string): Promise<void> {
    return api.invoke('relay_deploy_start', { connectionId });
  },

  /** Poll a detached task: incremental log output plus status. */
  async poll(
    connectionId: string,
    task: RelayDeployTask,
    cursor: number,
  ): Promise<RelayTaskPoll> {
    return api.invoke<RelayTaskPoll>('relay_deploy_poll', { connectionId, task, cursor });
  },

  /**
   * Provision a relay account locally and import it into the deployed relay.
   * The plaintext password never leaves this device.
   */
  async register(connectionId: string, username: string, password: string): Promise<void> {
    return api.invoke('relay_deploy_register', { connectionId, username, password });
  },

  /** Check the relay URL is reachable from this device (firewall/security-group check). */
  async verify(relayUrl: string): Promise<RelayVerifyResult> {
    return api.invoke<RelayVerifyResult>('relay_deploy_verify', { relayUrl });
  },
};
