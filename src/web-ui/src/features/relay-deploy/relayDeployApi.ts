/**
 * Relay Deploy Feature - API Service
 *
 * Wraps the desktop `relay_deploy_*` Tauri commands: deploy the open-source
 * BitFun relay server to a user-owned host over an existing SSH connection.
 */

import { api } from '@/infrastructure/api/service-api/ApiClient';

export type RelayDeployTask = 'install_docker' | 'deploy';
export type RelayTaskStatus = 'running' | 'succeeded' | 'failed';

export type DockerAccessMode =
  | 'ok'
  | 'group_inactive'
  | 'sudo_nopass'
  | 'sudo_needs_password'
  | 'broken_docker_home'
  | 'daemon_down'
  | 'missing';

export interface RelayPreflight {
  os: string;
  arch: string;
  archSupported: boolean;
  dockerInstalled: boolean;
  composeAvailable: boolean;
  /** Legacy coarse string: "ok" | "sudo" | "unreachable" */
  dockerDaemon: string;
  dockerAccessMode: DockerAccessMode;
  activeHasDockerGroup: boolean;
  inDockerGroupFile: boolean;
  dockerHomeWritable: boolean;
  tarAvailable: boolean;
  curlAvailable: boolean;
  sudoAvailable: boolean;
  sudoNeedsPassword: boolean;
  memTotalMb: number;
  portBusy: boolean;
  /** Port that was probed for busy/selected-port health checks. */
  probedPort: number;
  /** Selected port belongs to the existing bitfun-relay (not an unrelated process). */
  portOwnedByRelay: boolean;
  containerExists: boolean;
  /** bitfun-relay container is currently running. */
  containerRunning: boolean;
  /** Host port published by the running relay (0 if unknown). */
  existingRelayPort: number;
  /** Relay answers /health on the selected port and/or the existing container port. */
  relayHealthy: boolean;
  homeDir: string;
}

export interface RelayTaskStart {
  scriptPath: string;
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
  async preflight(connectionId: string, port?: number): Promise<RelayPreflight> {
    return api.invoke<RelayPreflight>('relay_deploy_preflight', {
      connectionId,
      port: port && port > 0 ? port : undefined,
    });
  },

  /** Stage the interactive Docker-install driver; run scriptPath in a remote PTY. */
  async installDocker(connectionId: string): Promise<RelayTaskStart> {
    return api.invoke<RelayTaskStart>('relay_deploy_install_docker', { connectionId });
  },

  /** Stage the interactive deploy driver; run scriptPath in a remote PTY. */
  async startDeploy(connectionId: string, port?: number): Promise<RelayTaskStart> {
    return api.invoke<RelayTaskStart>('relay_deploy_start', {
      connectionId,
      port: port && port > 0 ? port : undefined,
    });
  },

  /** Poll detached build status (marker/pid); PTY shows live output. */
  async poll(
    connectionId: string,
    task: RelayDeployTask,
    cursor: number,
  ): Promise<RelayTaskPoll> {
    return api.invoke<RelayTaskPoll>('relay_deploy_poll', { connectionId, task, cursor });
  },

  /** Stop a running install/deploy task (wizard closed or navigated away). */
  async cancel(connectionId: string, task: RelayDeployTask): Promise<void> {
    return api.invoke('relay_deploy_cancel', { connectionId, task });
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
