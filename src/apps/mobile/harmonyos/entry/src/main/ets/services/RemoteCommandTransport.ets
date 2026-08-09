import { CloudAccountClient, CloudAccountSession } from './CloudAccountClient';
import { CommandStatusResponse, RemoteCommand } from '../model/RemoteModels';
import { RelayHttpClient } from './RelayHttpClient';

export interface RemoteCommandTransport {
  send<T extends CommandStatusResponse>(command: RemoteCommand, timeoutMs?: number): Promise<T>;
  reset(): void;
}

export class RoomRemoteCommandTransport implements RemoteCommandTransport {
  private readonly client: RelayHttpClient;

  constructor(client: RelayHttpClient) {
    this.client = client;
  }

  async send<T extends CommandStatusResponse>(command: RemoteCommand, timeoutMs: number = 30000): Promise<T> {
    return this.client.sendCommand<T>(command, timeoutMs);
  }

  reset(): void {
    this.client.reset();
  }
}

export class AccountDeviceCommandTransport implements RemoteCommandTransport {
  private readonly client: CloudAccountClient;
  private readonly relayUrl: string;
  private readonly session: CloudAccountSession;
  private readonly targetDeviceId: string;

  constructor(
    client: CloudAccountClient,
    relayUrl: string,
    session: CloudAccountSession,
    targetDeviceId: string
  ) {
    this.client = client;
    this.relayUrl = relayUrl;
    this.session = session;
    this.targetDeviceId = targetDeviceId;
  }

  async send<T extends CommandStatusResponse>(command: RemoteCommand, _timeoutMs: number = 120000): Promise<T> {
    return this.client.deviceRpc<T>(this.relayUrl, this.session, this.targetDeviceId, command);
  }

  reset(): void {
    // Account credentials are owned by CloudAccountSessionStore, not by this transport.
  }
}
