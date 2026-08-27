import { CloudAccountClient, CloudAccountSession } from './CloudAccountClient';
import { CommandStatusResponse, RemoteCommand } from '../model/RemoteModels';
import { RelayHttpClient } from './RelayHttpClient';

/** Interactive directory rows must fail fast enough to leave a usable retry. */
export const INTERACTIVE_DIRECTORY_TIMEOUT_MS: number = 15000;

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

  /**
   * The default is the relay's read timeout, not the room transport's 30 s: an
   * account command crosses one more hop and the desktop may be waking a cold
   * process. It was previously ignored outright, so a caller asking for a
   * shorter deadline silently got two minutes — which is the whole wait a
   * failed device switch used to cost.
   */
  async send<T extends CommandStatusResponse>(command: RemoteCommand, timeoutMs: number = 130000): Promise<T> {
    return this.client.deviceRpc<T>(this.relayUrl, this.session, this.targetDeviceId, command, timeoutMs);
  }

  reset(): void {
    // Account credentials are owned by CloudAccountSessionStore, not by this transport.
  }
}
