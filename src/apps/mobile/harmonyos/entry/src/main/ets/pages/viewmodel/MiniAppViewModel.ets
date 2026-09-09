import { MiniAppActions, MiniAppBridgeReply, MiniAppBridgeRequest, RemoteMiniAppPayload, RemoteMiniAppClient } from '../../model/RemoteMiniAppModels';
import { MiniAppDocument } from '../../services/MiniAppDocument';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { MiniAppState } from '../state/MiniAppState';

export class MiniAppViewModel {
  readonly state: MiniAppState = new MiniAppState();
  private generation: number = 0;
  private connection: number = -1;
  private version: number = 0;
  private readonly client: RemoteMiniAppClient;
  readonly actions: MiniAppActions = {
    open: (): void => { void this.open(); },
    close: (): void => this.close(),
    back: (): void => { void this.open(); },
    refresh: (): void => { void this.open(); },
    select: (id: string): void => { void this.select(id); },
    request: (raw: string): Promise<string> => this.request(raw)
  };

  constructor(client: RemoteMiniAppClient) { this.client = client; }

  close(): void {
    this.generation += 1;
    this.state.visible = false;
    this.state.document = '';
    this.state.appId = '';
  }

  async open(): Promise<void> {
    const generation = ++this.generation;
    this.connection = this.client.miniAppConnection();
    this.state.visible = true;
    this.state.document = '';
    this.state.appId = '';
    this.state.name = '';
    this.state.apps = [];
    this.state.error = '';
    this.state.loading = true;
    try {
      const value = await this.client.miniApp({ action: 'list' }, this.connection) as RemoteMiniAppPayload;
      if (generation === this.generation) this.state.apps = value.apps || [];
    } catch (error) {
      if (generation === this.generation) this.state.error = String(error);
    } finally {
      if (generation === this.generation) this.state.loading = false;
    }
  }

  async select(id: string): Promise<void> {
    const generation = ++this.generation;
    this.state.loading = true;
    this.state.error = '';
    try {
      const value = await this.client.miniApp({ action: 'open', app_id: id }, this.connection) as RemoteMiniAppPayload;
      if (generation !== this.generation) return;
      if (!value.html || value.id !== id || value.version === undefined) throw new Error(RemoteI18n.t('miniapp.invalidPage'));
      this.version = value.version;
      this.state.appId = id;
      this.state.name = value.name || id;
      this.state.document = MiniAppDocument.build(value.html, RemoteI18n.language());
      this.state.revision += 1;
    } catch (error) {
      if (generation === this.generation) this.state.error = String(error);
    } finally {
      if (generation === this.generation) this.state.loading = false;
    }
  }

  async request(raw: string): Promise<string> {
    const generation = this.generation;
    let id: string = '';
    try {
      const rpc = JSON.parse(raw) as MiniAppBridgeRequest;
      id = rpc.id;
      if (typeof id !== 'string' || rpc.method !== 'worker.call' || !rpc.params ||
        typeof rpc.params.method !== 'string' || !this.state.visible || !this.state.appId) {
        throw new Error(RemoteI18n.t('miniapp.unsupported'));
      }
      const result = await this.client.miniApp({ action: 'call', app_id: this.state.appId,
        version: this.version, method: rpc.params.method, params: rpc.params.params }, this.connection);
      if (generation !== this.generation) throw new Error(RemoteI18n.t('miniapp.closed'));
      const reply: MiniAppBridgeReply = { jsonrpc: '2.0', id, result: result === undefined ? null : result };
      return JSON.stringify(reply);
    } catch (error) {
      const reply: MiniAppBridgeReply = { jsonrpc: '2.0', id, error: { message: String(error) } };
      return JSON.stringify(reply);
    }
  }
}
