/**
 * Manages remote sessions by sending commands to the desktop via the relay.
 * All communication is request-response via RelayHttpClient (HTTP).
 *
 * Includes SessionPoller for incremental state synchronization:
 *   - Active tab: poll every 1 second
 *   - Inactive tab: poll every 5 seconds
 *   - On tab activation: immediate poll to catch up on missed changes
 */

import { RelayHttpClient } from './RelayHttpClient';

export interface WorkspaceInfo {
  has_workspace: boolean;
  path?: string;
  project_name?: string;
  git_branch?: string;
  /** Mirrors desktop `WorkspaceKind`: normal project, Claw assistant workspace, or remote SSH. */
  workspace_kind?: 'normal' | 'assistant' | 'remote';
  assistant_id?: string;
}

export interface RecentWorkspaceEntry {
  path: string;
  name: string;
  last_opened: string;
  workspace_kind?: 'normal' | 'assistant' | 'remote';
}

export interface AssistantEntry {
  path: string;
  name: string;
  assistant_id?: string;
}

export interface SessionInfo {
  session_id: string;
  name: string;
  agent_type: string;
  created_at: string;
  updated_at: string;
  message_count: number;
  workspace_path?: string;
  workspace_name?: string;
}

export interface RemoteModelConfig {
  id: string;
  name: string;
  provider: string;
  base_url: string;
  model_name: string;
  context_window?: number;
  enabled: boolean;
  capabilities: string[];
  enable_thinking_process?: boolean;
  reasoning_mode?: 'default' | 'enabled' | 'disabled' | 'adaptive';
  reasoning_effort?: string;
}

export interface RemoteDefaultModels {
  primary?: string | null;
  fast?: string | null;
}

export interface RemoteModelCatalog {
  version: number;
  models: RemoteModelConfig[];
  default_models: RemoteDefaultModels;
  session_model_id?: string | null;
}

export interface ChatMessageItem {
  type: 'text' | 'tool' | 'thinking';
  content?: string;
  tool?: RemoteToolStatus;
  is_subagent?: boolean;
  subItems?: ChatMessageItem[];
}

export interface ChatImageAttachment {
  name: string;
  data_url: string;
}

export interface ChatMessage {
  id: string;
  role: string;
  content: string;
  timestamp: string;
  metadata?: any;
  tools?: RemoteToolStatus[];
  thinking?: string;
  items?: ChatMessageItem[];
  images?: ChatImageAttachment[];
}

export interface ActiveTurnSnapshot {
  turn_id: string;
  status: string;
  text: string;
  thinking: string;
  tools: RemoteToolStatus[];
  round_index: number;
  items?: ChatMessageItem[];
}

export interface RemoteToolStatus {
  id: string;
  name: string;
  status: string;
  duration_ms?: number;
  start_ms?: number;
  input_preview?: string;
  tool_input?: any;
}

export interface PollResponse {
  resp: string;
  version: number;
  changed: boolean;
  session_state?: string;
  title?: string;
  new_messages?: ChatMessage[];
  total_msg_count?: number;
  active_turn?: ActiveTurnSnapshot | null;
  model_catalog?: RemoteModelCatalog;
}

export interface InitialSyncData {
  has_workspace: boolean;
  path?: string;
  project_name?: string;
  git_branch?: string;
  workspace_kind?: 'normal' | 'assistant' | 'remote';
  assistant_id?: string;
  sessions: SessionInfo[];
  has_more_sessions: boolean;
  authenticated_user_id?: string;
}

export class RemoteSessionManager {
  private client: RelayHttpClient;

  constructor(client: RelayHttpClient) {
    this.client = client;
  }

  private async request<T>(cmd: object): Promise<T> {
    const requestId = `req_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const cmdWithId = { ...cmd, _request_id: requestId };
    const resp = await this.client.sendCommand<T>(cmdWithId);
    const respAny = resp as any;
    if (respAny.resp === 'error') {
      throw new Error(respAny.message || 'Unknown error');
    }
    return resp;
  }

  async getWorkspaceInfo(): Promise<WorkspaceInfo> {
    const resp = await this.request<{ resp: string } & WorkspaceInfo>({
      cmd: 'get_workspace_info',
    });
    return {
      has_workspace: resp.has_workspace,
      path: resp.path,
      project_name: resp.project_name,
      git_branch: resp.git_branch,
      workspace_kind: resp.workspace_kind,
      assistant_id: resp.assistant_id,
    };
  }

  async listRecentWorkspaces(): Promise<RecentWorkspaceEntry[]> {
    const resp = await this.request<{
      resp: string;
      workspaces: RecentWorkspaceEntry[];
    }>({ cmd: 'list_recent_workspaces' });
    return resp.workspaces || [];
  }

  async setWorkspace(
    path: string,
  ): Promise<{
    success: boolean;
    path?: string;
    project_name?: string;
    error?: string;
  }> {
    return this.request({ cmd: 'set_workspace', path });
  }

  async listAssistants(): Promise<AssistantEntry[]> {
    const resp = await this.request<{
      resp: string;
      assistants: AssistantEntry[];
    }>({ cmd: 'list_assistants' });
    return resp.assistants || [];
  }

  async setAssistant(
    path: string,
  ): Promise<{
    success: boolean;
    path?: string;
    name?: string;
    error?: string;
  }> {
    return this.request({ cmd: 'set_assistant', path });
  }

  async listSessions(
    workspacePath?: string,
    limit = 30,
    offset = 0,
  ): Promise<{ sessions: SessionInfo[]; has_more: boolean }> {
    const resp = await this.request<{
      resp: string;
      sessions: SessionInfo[];
      has_more: boolean;
    }>({
      cmd: 'list_sessions',
      workspace_path: workspacePath ?? null,
      limit,
      offset,
    });
    return {
      sessions: resp.sessions || [],
      has_more: resp.has_more ?? false,
    };
  }

  async createSession(
    agentType?: string,
    sessionName?: string,
    workspacePath?: string,
  ): Promise<string> {
    const resp = await this.request<{ resp: string; session_id: string }>({
      cmd: 'create_session',
      agent_type: agentType || undefined,
      session_name: sessionName || undefined,
      workspace_path: workspacePath ?? null,
    });
    return resp.session_id;
  }

  async getSessionMessages(
    sessionId: string,
    limit?: number,
    beforeId?: string,
  ): Promise<{ messages: ChatMessage[]; has_more: boolean }> {
    const resp = await this.request<{
      resp: string;
      messages: ChatMessage[];
      has_more: boolean;
    }>({
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

  async getModelCatalog(sessionId?: string): Promise<RemoteModelCatalog> {
    const resp = await this.request<{
      resp: string;
      catalog: RemoteModelCatalog;
    }>({
      cmd: 'get_model_catalog',
      session_id: sessionId ?? undefined,
    });
    return resp.catalog;
  }

  async setSessionModel(sessionId: string, modelId: string): Promise<string> {
    const resp = await this.request<{
      resp: string;
      session_id: string;
      model_id: string;
    }>({
      cmd: 'set_session_model',
      session_id: sessionId,
      model_id: modelId,
    });
    return resp.model_id;
  }

  async sendMessage(
    sessionId: string,
    content: string,
    agentType?: string,
    imageContexts?: Array<{
      id: string;
      image_path?: string;
      data_url?: string;
      mime_type: string;
      metadata?: Record<string, unknown>;
    }>,
  ): Promise<string> {
    const resp = await this.request<{ resp: string; turn_id: string }>({
      cmd: 'send_message',
      session_id: sessionId,
      content,
      agent_type: agentType || undefined,
      image_contexts: imageContexts && imageContexts.length > 0 ? imageContexts : undefined,
    });
    return resp.turn_id;
  }

  async cancelTask(sessionId: string, turnId?: string): Promise<void> {
    await this.request({
      cmd: 'cancel_task',
      session_id: sessionId,
      turn_id: turnId ?? undefined,
    });
  }

  async cancelTool(toolId: string, reason?: string): Promise<void> {
    await this.request({
      cmd: 'cancel_tool',
      tool_id: toolId,
      reason: reason ?? undefined,
    });
  }

  async deleteSession(sessionId: string): Promise<void> {
    await this.request({ cmd: 'delete_session', session_id: sessionId });
  }

  async answerQuestion(toolId: string, answers: any): Promise<void> {
    await this.request({ cmd: 'answer_question', tool_id: toolId, answers });
  }

  async pollSession(
    sessionId: string,
    sinceVersion: number,
    knownMsgCount: number,
    knownModelCatalogVersion = 0,
  ): Promise<PollResponse> {
    return this.request<PollResponse>({
      cmd: 'poll_session',
      session_id: sessionId,
      since_version: sinceVersion,
      known_msg_count: knownMsgCount,
      known_model_catalog_version: knownModelCatalogVersion,
    });
  }

  async ping(): Promise<void> {
    await this.request({ cmd: 'ping' });
  }

  /**
   * Fetch metadata for a workspace file (name, size, MIME type) without
   * transferring its content.  Used to render file cards before the user
   * confirms a download.
   */
  async getFileInfo(path: string, sessionId?: string): Promise<{
    name: string;
    size: number;
    mimeType: string;
  }> {
    const resp = await this.request<{
      resp: string;
      name: string;
      size: number;
      mime_type: string;
    }>({ cmd: 'get_file_info', path, session_id: sessionId ?? undefined });
    return {
      name: resp.name,
      size: resp.size,
      mimeType: resp.mime_type,
    };
  }

  /**
   * Read a workspace file using chunked transfer.
   *
   * Downloads the file in 4 MB chunks, reassembles the base64 pieces, and
   * calls `onProgress(downloaded, total)` after each chunk so the UI can
   * display a progress bar.
   */
  async readFile(
    path: string,
    sessionId?: string,
    onProgress?: (downloaded: number, total: number) => void,
  ): Promise<{
    name: string;
    contentBase64: string;
    mimeType: string;
    size: number;
  }> {
    // Must be divisible by 3 so intermediate base64 chunks have no `=` padding;
    // joining padded chunks would produce invalid base64 for `atob()`.
    const CHUNK_SIZE = 3 * 1024 * 1024; // 3 MB per request
    let offset = 0;
    const chunks: string[] = [];
    let fileName = '';
    let mimeType = '';
    let totalSize = 0;

    // eslint-disable-next-line no-constant-condition
    while (true) {
      const resp = await this.request<{
        resp: string;
        name: string;
        chunk_base64: string;
        offset: number;
        chunk_size: number;
        total_size: number;
        mime_type: string;
      }>({
        cmd: 'read_file_chunk',
        path,
        session_id: sessionId ?? undefined,
        offset,
        limit: CHUNK_SIZE,
      });

      chunks.push(resp.chunk_base64);
      fileName = resp.name;
      mimeType = resp.mime_type;
      totalSize = resp.total_size;
      offset += resp.chunk_size;

      onProgress?.(Math.min(offset, totalSize), totalSize);

      if (offset >= totalSize || resp.chunk_size === 0) break;
    }

    return {
      name: fileName,
      contentBase64: chunks.join(''),
      mimeType,
      size: totalSize,
    };
  }
}

// ── SessionPoller ─────────────────────────────────────────────────

export class SessionPoller {
  private intervalId: ReturnType<typeof setTimeout> | null = null;
  private sinceVersion = 0;
  private knownMsgCount = 0;
  private sessionId: string;
  private sessionMgr: RemoteSessionManager;
  private onUpdate: (state: PollResponse) => void;
  private polling = false;
  private stopped = false;
  private hasActiveTurn = false;
  private knownModelCatalogVersion = 0;
  private turnJustEndedAt: number | null = null;
  private readonly TURN_JUST_ENDED_GRACE_PERIOD_MS = 5000;

  constructor(
    sessionMgr: RemoteSessionManager,
    sessionId: string,
    onUpdate: (state: PollResponse) => void,
    initialModelCatalogVersion = 0,
  ) {
    this.sessionMgr = sessionMgr;
    this.sessionId = sessionId;
    this.onUpdate = onUpdate;
    this.knownModelCatalogVersion = initialModelCatalogVersion;
  }

  start(initialMsgCount = 0) {
    this.stopped = false;
    this.knownMsgCount = initialMsgCount;
    this.tick();
    document.addEventListener('visibilitychange', this.onVisibilityChange);
  }

  stop() {
    this.stopped = true;
    if (this.intervalId !== null) {
      clearTimeout(this.intervalId);
      this.intervalId = null;
    }
    document.removeEventListener('visibilitychange', this.onVisibilityChange);
  }

  resetCursors() {
    this.sinceVersion = 0;
    this.knownMsgCount = 0;
  }

  setKnownModelCatalogVersion(version: number) {
    this.knownModelCatalogVersion = version;
  }

  /** Call after sending a message to immediately switch to fast polling. */
  nudge() {
    this.hasActiveTurn = true;
    if (this.intervalId !== null) clearTimeout(this.intervalId);
    this.tick();
  }

  private getInterval(): number {
    if (document.visibilityState !== 'visible') return 5000;
    
    // During grace period after a turn just ended, keep fast polling
    const now = Date.now();
    if (this.turnJustEndedAt != null && (now - this.turnJustEndedAt) < this.TURN_JUST_ENDED_GRACE_PERIOD_MS) {
      return 1000;
    }
    
    return this.hasActiveTurn ? 1000 : 10000;
  }

  private scheduleNext() {
    if (this.stopped) return;
    if (this.intervalId !== null) clearTimeout(this.intervalId);
    this.intervalId = setTimeout(() => this.tick(), this.getInterval());
  }

  private onVisibilityChange = () => {
    if (this.stopped) return;
    if (document.visibilityState === 'visible') {
      if (this.intervalId !== null) clearTimeout(this.intervalId);
      this.tick();
    } else {
      this.scheduleNext();
    }
  };

  private async tick() {
    if (this.stopped || this.polling) {
      this.scheduleNext();
      return;
    }
    this.polling = true;
    try {
      const resp = await this.sessionMgr.pollSession(
        this.sessionId,
        this.sinceVersion,
        this.knownMsgCount,
        this.knownModelCatalogVersion,
      );
      // Only update hasActiveTurn from responses that carry actual data.
      // When changed=false the backend omits active_turn, so we must
      // preserve the previous value to keep 1-second fast polling alive.
      if (resp.changed) {
        const wasActive = this.hasActiveTurn;
        const isActiveNow = resp.active_turn != null && resp.active_turn.status === 'active';
        this.hasActiveTurn = isActiveNow;
        
        // active_turn just ended — start grace period
        if (wasActive && !isActiveNow) {
          this.turnJustEndedAt = Date.now();
        }
        
        // New messages or still-active turn clears the grace period
        if (resp.new_messages && resp.new_messages.length > 0) {
          this.turnJustEndedAt = null;
        }
        
        this.sinceVersion = resp.version;
        if (resp.total_msg_count != null) {
          this.knownMsgCount = resp.total_msg_count;
        }
        if (resp.model_catalog?.version != null) {
          this.knownModelCatalogVersion = resp.model_catalog.version;
        }
        this.onUpdate(resp);
      }
    } catch (e) {
      console.error('[Poller] poll error', e);
    } finally {
      this.polling = false;
      this.scheduleNext();
    }
  }
}
