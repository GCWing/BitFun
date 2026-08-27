import {
  AssistantEntry,
  AssistantListResponse,
  RecentWorkspaceEntry,
  RecentWorkspaceListResponse,
  RemoteSession,
  SessionListResponse
} from '../model/RemoteModels';
import { CloudAccountBinding } from './CloudAccountClient';
import {
  AccountDeviceCommandTransport,
  INTERACTIVE_DIRECTORY_TIMEOUT_MS
} from './RemoteCommandTransport';
import { RemoteCommandFactory } from './RemoteCommandFactory';
import { RemoteResponseMapper } from './RemoteResponseMapper';
import { stampRemoteSessionsDevice, stampRemoteSessionsWorkspace } from './RemoteSessionIdentity';

export interface AccountDeviceDirectorySource {
  listRecentWorkspaces(deviceId: string): Promise<RecentWorkspaceEntry[]>;
  listAssistants(deviceId: string): Promise<AssistantEntry[]>;
  listSessions(deviceId: string, workspacePath: string): Promise<RemoteSession[]>;
}

/** Read-only per-device RPCs that never touch the active conversation transport. */
export class AccountDeviceDirectory implements AccountDeviceDirectorySource {
  private readonly context: CloudAccountBinding;

  constructor(context: CloudAccountBinding) {
    this.context = context;
  }

  async listRecentWorkspaces(deviceId: string): Promise<RecentWorkspaceEntry[]> {
    const response = await this.transport(deviceId).send<RecentWorkspaceListResponse>(
      RemoteCommandFactory.listRecentWorkspaces()
    );
    return RemoteResponseMapper.recentWorkspaces(response.workspaces || []);
  }

  async listAssistants(deviceId: string): Promise<AssistantEntry[]> {
    const response = await this.transport(deviceId).send<AssistantListResponse>(
      RemoteCommandFactory.listAssistants()
    );
    return response.assistants || [];
  }

  async listSessions(deviceId: string, workspacePath: string): Promise<RemoteSession[]> {
    const path = workspacePath.trim();
    if (path.length === 0) {
      return [];
    }
    const response = await this.transport(deviceId).send<SessionListResponse>(
      RemoteCommandFactory.listSessions(path, 50, 0, ''),
      INTERACTIVE_DIRECTORY_TIMEOUT_MS
    );
    const sessions = stampRemoteSessionsWorkspace(
      RemoteResponseMapper.sessions(response.sessions || []),
      path
    );
    return stampRemoteSessionsDevice(sessions, deviceId);
  }

  private transport(deviceId: string): AccountDeviceCommandTransport {
    return new AccountDeviceCommandTransport(
      this.context.client,
      this.context.relayUrl,
      this.context.session,
      deviceId
    );
  }
}
