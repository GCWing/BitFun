/**
 * Manages remote sessions by sending commands to the desktop via the relay.
 *
 * Response delivery uses a dual mechanism:
 *   1. WebSocket real-time relay (onMessage callback from RelayConnection)
 *   2. HTTP polling (RelayConnection.pollMessages) for missed messages
 *
 * Polling is started automatically after construction and runs every 2 seconds.
 * Both paths feed into the same handleMessage() dispatcher.
 */

import { RelayConnection } from './RelayConnection';

export interface WorkspaceInfo {
  has_workspace: boolean;
  path?: string;
  project_name?: string;
  git_branch?: string;
}

export interface RecentWorkspaceEntry {
  path: string;
  name: string;
  last_opened: string;
}

export interface SessionInfo {
  session_id: string;
  name: string;
  agent_type: string;
  created_at: string;
  updated_at: string;
  message_count: number;
}

export interface ChatMessage {
  id: string;
  role: string;
  content: string;
  timestamp: string;
  metadata?: any;
}

export class RemoteSessionManager {
  private relay: RelayConnection;
  private pendingCallbacks = new Map<string, (data: any) => void>();
  private streamListeners: ((event: any) => void)[] = [];

  constructor(relay: RelayConnection) {
    this.relay = relay;
    // Start polling for buffered messages from the relay server
    this.relay.startPolling(2000);
  }

  onStreamEvent(listener: (event: any) => void) {
    this.streamListeners.push(listener);
    return () => {
      this.streamListeners = this.streamListeners.filter(l => l !== listener);
    };
  }

  handleMessage(json: string) {
    try {
      const msg = JSON.parse(json);

      if (msg.resp === 'stream_event') {
        this.streamListeners.forEach(l => l(msg));
        return;
      }

      if (msg._request_id && this.pendingCallbacks.has(msg._request_id)) {
        const cb = this.pendingCallbacks.get(msg._request_id)!;
        this.pendingCallbacks.delete(msg._request_id);
        cb(msg);
      }
    } catch (e) {
      console.error('[SessionMgr] Failed to parse message', e);
    }
  }

  private async request<T>(cmd: object): Promise<T> {
    const requestId = `req_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const cmdWithId = { ...cmd, _request_id: requestId };

    return new Promise<T>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingCallbacks.delete(requestId);
        reject(new Error('Request timeout'));
      }, 30_000);

      this.pendingCallbacks.set(requestId, (data) => {
        clearTimeout(timeout);
        if (data.resp === 'error') {
          reject(new Error(data.message));
        } else {
          resolve(data as T);
        }
      });

      this.relay.sendCommand(cmdWithId).catch(reject);
    });
  }

  async getWorkspaceInfo(): Promise<WorkspaceInfo> {
    const resp = await this.request<{ resp: string } & WorkspaceInfo>({ cmd: 'get_workspace_info' });
    return {
      has_workspace: resp.has_workspace,
      path: resp.path,
      project_name: resp.project_name,
      git_branch: resp.git_branch,
    };
  }

  async listRecentWorkspaces(): Promise<RecentWorkspaceEntry[]> {
    const resp = await this.request<{ resp: string; workspaces: RecentWorkspaceEntry[] }>({
      cmd: 'list_recent_workspaces',
    });
    return resp.workspaces || [];
  }

  async setWorkspace(path: string): Promise<{ success: boolean; path?: string; project_name?: string; error?: string }> {
    const resp = await this.request<{
      resp: string;
      success: boolean;
      path?: string;
      project_name?: string;
      error?: string;
    }>({ cmd: 'set_workspace', path });
    return resp;
  }

  async listSessions(): Promise<SessionInfo[]> {
    const resp = await this.request<{ resp: string; sessions: SessionInfo[] }>({ cmd: 'list_sessions' });
    return resp.sessions || [];
  }

  async createSession(agentType?: string, sessionName?: string): Promise<string> {
    const resp = await this.request<{ resp: string; session_id: string }>({
      cmd: 'create_session',
      agent_type: agentType || undefined,
      session_name: sessionName || undefined,
    });
    return resp.session_id;
  }

  async getSessionMessages(
    sessionId: string,
    limit?: number,
    beforeId?: string
  ): Promise<{ messages: ChatMessage[]; has_more: boolean }> {
    const resp = await this.request<{ resp: string; messages: ChatMessage[]; has_more: boolean }>({
      cmd: 'get_session_messages',
      session_id: sessionId,
      limit,
      before_message_id: beforeId,
    });
    return {
      messages: resp.messages || [],
      has_more: resp.has_more || false,
    };
  }

  async subscribeSession(sessionId: string): Promise<void> {
    await this.request({ cmd: 'subscribe_session', session_id: sessionId });
  }

  async unsubscribeSession(sessionId: string): Promise<void> {
    await this.request({ cmd: 'unsubscribe_session', session_id: sessionId });
  }

  async sendMessage(sessionId: string, content: string): Promise<string> {
    const resp = await this.request<{ resp: string; turn_id: string }>({
      cmd: 'send_message',
      session_id: sessionId,
      content,
    });
    return resp.turn_id;
  }

  async cancelTask(sessionId: string): Promise<void> {
    await this.request({ cmd: 'cancel_task', session_id: sessionId });
  }

  async deleteSession(sessionId: string): Promise<void> {
    await this.request({ cmd: 'delete_session', session_id: sessionId });
  }

  async ping(): Promise<void> {
    await this.request({ cmd: 'ping' });
  }

  dispose() {
    this.relay.stopPolling();
    this.pendingCallbacks.clear();
    this.streamListeners = [];
  }
}
