 

import { ITransportAdapter } from './base';
import { createLogger } from '@/shared/utils/logger';
import { getVersionInfo } from '@/shared/utils/version';
import { projectAgenticEventEnvelope } from '@/shared/utils/agenticEventProjection';
import type {
  AgentSessionArchiveStateRequest,
  AgentSessionForkAtTurnRequest,
  ConfigUpdate,
  ForkSessionResponse,
  GitBranch,
  GitRepositoryPathRequest,
  ListSessionsResponse,
  PermissionGrant,
  PermissionReply,
  RemoveProjectPermissionGrantResponse,
  ResetAgentProfileConfigMessage,
  ResetAgentProfileConfigResponse,
  RunResponse,
  SetAgentProfileConfigMessage,
  SetAgentProfileConfigResponse,
  SubmitDialogTurnBody,
  SubmitDialogTurnResponse,
} from '@/generated/api';

const log = createLogger('WebSocketAdapter');
const APP_SERVER_PROTOCOL_VERSION = 4;
const APP_SERVER_INITIALIZE_METHOD = 'app/initialize';
const APP_SERVER_INITIALIZE_CAPABILITY = 'app.initialize';
const APP_SERVER_SYNC_EVENTS_METHOD = 'app/syncEvents';
export const SESSION_OWNER_REFRESH_EVENT = 'app-server://session-owner-refresh-required';
const SESSION_OWNER_REFRESH_ERROR =
  'App Server state or a side-effect outcome cannot be recovered safely. Reload the Web page before retrying.';

/**
 * Typed mapping from the frontend's snake_case agent commands to the app-server
 * JSON-RPC method names, carrying the request/response types from the generated
 * schema (`@/generated/api`, source: `bitfun-app-server-protocol`).
 *
 * The service layer (`AgentAPI` and friends) speaks Tauri command names
 * (`create_session`, `start_dialog_turn`, ...) because that is the desktop
 * contract. In web mode the request rides the websocket to the Server Host,
 * whose dispatch (`src/apps/server/src/routes/websocket.rs`) only recognizes the
 * app-server surface methods (`agent/createSession`, ...). This table is the one
 * place that bridges the two naming conventions, so the typed service API stays
 * transport-agnostic and desktop/web share identical call sites.
 *
 * `AGENT_COMMAND_TO_WS_METHOD` (below) is derived from this table so the
 * exhaustive-and-stable test stays green and the snake->method mapping remains a
 * single source. The `request`/`response` slots are type-only (no runtime
 * value); they back the `request<K>()` overload so callers that use a known
 * command key get schema-typed bodies and responses.
 *
 * NOTE(Step 1): the typed `request`/`response` slots reflect the **schema wire
 * shape**. For `start_dialog_turn` the frontend currently sends the desktop host
 * shape (`userInput`/`dialogTurnId`/`imageContexts`) and `websocket.rs`
 * normalizes it to the schema shape (`message`/`turnId`/`attachments`) -- that
 * normalizer is removed in Step 2 once call sites migrate to the schema shape.
 * Until then the slot is annotated with the schema type so the drift is
 * type-visible.
 */
interface AgentCommandEntry {
  method: string;
  request?: unknown;
  response?: unknown;
  sideEffecting?: true;
}

export const AGENT_COMMAND_SCHEMA = {
  create_session: { method: 'agent/createSession', sideEffecting: true },
  list_sessions: {
    method: 'agent/listSessions',
    response: null as unknown as ListSessionsResponse,
  },
  delete_session: { method: 'agent/deleteSession', sideEffecting: true },
  fork_session: {
    method: 'session/forkAtTurn',
    sideEffecting: true,
    request: null as unknown as AgentSessionForkAtTurnRequest,
    response: null as unknown as ForkSessionResponse,
  },
  archive_session: {
    method: 'session/setArchived',
    sideEffecting: true,
    request: null as unknown as AgentSessionArchiveStateRequest,
  },
  unarchive_session: {
    method: 'session/setArchived',
    sideEffecting: true,
    request: null as unknown as AgentSessionArchiveStateRequest,
  },
  // `start_dialog_turn` maps to `agent/submitDialogTurn`, not `agent/submitTurn`:
  // the dialog-turn body carries `agentType`/`workspacePath`/`policy`, which
  // the bare submission request does not. This mirrors the desktop host, which
  // drives `AgentRuntime::submit_dialog_turn` from the `start_dialog_turn`
  // Tauri command (`agentic_api.rs`).
  start_dialog_turn: {
    method: 'agent/submitDialogTurn',
    sideEffecting: true,
    request: null as unknown as SubmitDialogTurnBody,
    response: null as unknown as SubmitDialogTurnResponse,
  },
  cancel_dialog_turn: {
    method: 'agent/cancelTurn',
    sideEffecting: true,
    response: null as unknown as RunResponse,
  },
  // Permission surface: the reply/list/grants operations map to the app-server
  // permission methods. `subscribe_permission_requests` has no direct
  // counterpart -- in web mode the app-server pushes typed
  // `agent/permissionEvent` notifications, which this adapter reconciles with
  // `app/syncEvents` and projects to `permission://event`.
  respond_permission: {
    method: 'agent/respondPermission',
    sideEffecting: true,
    request: null as unknown as { request_id: string; reply: PermissionReply },
  },
  respond_permission_batch: {
    method: 'agent/respondPermissionBatch',
    sideEffecting: true,
    request: null as unknown as { request_id: string; reply: PermissionReply },
  },
  list_pending_permission_requests: {
    method: 'agent/listPendingPermissionRequests',
  },
  subscribe_permission_requests: {
    method: 'agent/listPendingPermissionRequests',
  },
  list_project_permission_grants: {
    method: 'agent/listProjectPermissionGrants',
    request: null as unknown as { project_id: string },
    response: null as unknown as { grants: PermissionGrant[] },
  },
  remove_project_permission_grant: {
    method: 'agent/removeProjectPermissionGrant',
    sideEffecting: true,
    request: null as unknown as PermissionGrant,
    response: null as unknown as RemoveProjectPermissionGrantResponse,
  },
  clear_project_permission_grants: {
    method: 'agent/clearProjectPermissionGrants',
    sideEffecting: true,
    request: null as unknown as { project_id: string },
  },
  // Git service surface (read-only in this batch). The app-server schema uses
  // the `group/verb` camelCase convention (`git/getStatus`) matching
  // `agent/createSession`; the frontend `GitAPI` call sites speak the
  // snake_case Tauri command names (`git_get_status`), so this map is the one
  // place that bridges the two. Write operations and the remote (SSH) path
  // arrive in later batches; the Server Host has no SSH manager, so remote git
  // paths surface as `host_capability_unavailable` (the `external_sources`
  // precedent).
  git_is_repository: {
    method: 'git/isRepository',
    request: null as unknown as GitRepositoryPathRequest,
  },
  git_get_status: {
    method: 'git/getStatus',
    request: null as unknown as GitRepositoryPathRequest,
  },
  git_get_branches: {
    method: 'git/getBranches',
    request: null as unknown as GitRepositoryPathRequest,
    response: null as unknown as { branches: GitBranch[] },
  },
  // Config service surface. The agent-profile and
  // model-config reads reach the global config singletons the Desktop host
  // also uses -- no service injection, mirroring the static `GitService`
  // pattern. `get_config`/`get_configs` carry the not-found -> undefined
  // contract the frontend `ConfigAPI` depends on: the app-server
  // `config_get_error` helper puts the `BitFunError::NotFound` Display text
  // into the JSON-RPC `message`, so `ConfigAPI.getConfig`'s substring match
  // (`not found:` + `config path` + `'<path>'`) hits and swallows the error
  // the same way it does on desktop. `get_skill_configs` (workspace
  // dependency) still lands in a later batch.
  get_agent_profile_configs: { method: 'config/getAgentProfileConfigs' },
  get_agent_profile_config: { method: 'config/getAgentProfileConfig' },
  get_model_configs: { method: 'config/getModelConfigs' },
  project_ai_model_reasoning_catalog: { method: 'model/projectReasoningCatalog' },
  get_config: { method: 'config/getConfig' },
  get_configs: { method: 'config/getConfigs' },
  set_agent_profile_config: {
    method: 'config/setAgentProfileConfig',
    sideEffecting: true,
    request: null as unknown as SetAgentProfileConfigMessage,
    response: null as unknown as SetAgentProfileConfigResponse,
  },
  reset_agent_profile_config: {
    method: 'config/resetAgentProfileConfig',
    sideEffecting: true,
    request: null as unknown as ResetAgentProfileConfigMessage,
    response: null as unknown as ResetAgentProfileConfigResponse,
  },
  set_config: { method: 'config/setConfig', sideEffecting: true },
  save_cloud_speech_config: { method: 'config/saveCloudSpeechConfig', sideEffecting: true },
  validate_config: { method: 'config/validateConfig' },
  i18n_get_current_language: { method: 'i18n/getCurrentLanguage' },
  i18n_set_language: { method: 'i18n/setLanguage', sideEffecting: true },
  i18n_get_config: { method: 'i18n/getConfig' },
  i18n_set_config: { method: 'i18n/setConfig', sideEffecting: true },
  i18n_get_supported_languages: { method: 'i18n/getSupportedLanguages' },
} as const satisfies Record<string, AgentCommandEntry>;

export type AgentCommandKey = keyof typeof AGENT_COMMAND_SCHEMA;

/**
 * Resolve the app-server JSON-RPC method name for a frontend command. The
 * snake_case Tauri command names the service-API call sites use (e.g.
 * `create_session`) map to the schema `group/verb` method names via the typed
 * `AGENT_COMMAND_SCHEMA` table; any command not in the table passes through
 * unchanged (desktop-only commands, `ping`, ...) and surfaces as
 * `method_not_found` on the browser-direct ACP path.
 *
 * Step 2b: the snake->method resolution is **web-transport-only** -- the desktop
 * and CLI hosts still call Tauri commands by their snake_case names directly,
 * so this indirection is confined to the WS adapter and does not affect them.
 */
export function resolveWsMethod(action: string): string {
  const entry = (AGENT_COMMAND_SCHEMA as Record<string, AgentCommandEntry>)[action];
  return entry?.method ?? action;
}

// ---------------------------------------------------------------------------
// Protocol conversion layer (Review2-Item2)
// ---------------------------------------------------------------------------
// The frontend service-API call sites speak the desktop Tauri command shapes
// (camelCase field names, bare-string permission replies, success/message
// responses, bare-array returns). The app-server JSON-RPC schema uses a
// different wire shape per method (schema snake_case fields, tagged-enum
// PermissionReply, status/sessionId/turnId responses, wrapper structs for
// collection results). The adapter is the single place that bridges the two so
// call sites stay transport-agnostic. Each encoder/decoder is pure and
// side-effect free; an unknown action falls through unchanged.

/**
 * Encodes the frontend request body into the app-server JSON-RPC wire shape for
 * the given command. Handles field-name and structural mismatches the desktop
 * host does in Rust (`agentic_api.rs`).
 */
export function encodeRequestBody(action: string, body: any): any {
  if (!body || typeof body !== 'object') return body ?? {};
  switch (action) {
    case 'start_dialog_turn': {
      const { userInput, originalUserInput, imageContexts, userMessageMetadata, ...rest } = body;
      return {
        ...rest,
        message: userInput,
        ...(originalUserInput !== undefined ? { originalMessage: originalUserInput } : {}),
        ...(Array.isArray(imageContexts) && imageContexts.length > 0
          ? { attachments: imageContexts.map(encodeImageAttachment) }
          : {}),
        ...(userMessageMetadata !== undefined ? { metadata: encodeUserMetadata(userMessageMetadata) } : {}),
      };
    }
    case 'respond_permission':
    case 'respond_permission_batch':
      return encodePermissionResponseBody(body);
    case 'fork_session':
      return {
        workspacePath: body.workspace_path,
        sourceSessionId: body.source_session_id,
        sourceTurnId: body.source_turn_id,
        ...(body.remote_connection_id !== undefined
          ? { remoteConnectionId: body.remote_connection_id }
          : {}),
        ...(body.remote_ssh_host !== undefined
          ? { remoteSshHost: body.remote_ssh_host }
          : {}),
      };
    case 'archive_session':
    case 'unarchive_session':
      return {
        workspacePath: body.workspace_path,
        sessionId: body.session_id,
        archived: action === 'archive_session',
        ...(body.remote_connection_id !== undefined
          ? { remoteConnectionId: body.remote_connection_id }
          : {}),
        ...(body.remote_ssh_host !== undefined
          ? { remoteSshHost: body.remote_ssh_host }
          : {}),
      };
    default:
      return body;
  }
}

/**
 * Decodes the app-server JSON-RPC result into the frontend-expected shape.
 * Handles response wrapper structs (bare-array unwrapping) and the
 * start_dialog_turn status-enum -> success/message projection.
 */
export function decodeResponseBody(action: string, result: any): any {
  switch (action) {
    case 'start_dialog_turn': {
      if (result && typeof result === 'object' && 'status' in result) {
        return { success: true, message: result.status === 'queued' ? 'Dialog turn queued' : 'Dialog turn started' };
      }
      return result;
    }
    case 'list_sessions':
      return unwrapArray(result, 'sessions');
    case 'list_pending_permission_requests':
    case 'subscribe_permission_requests':
      return unwrapArray(result, 'requests');
    case 'respond_permission_batch':
      return unwrapArray(result, 'request_ids');
    case 'list_project_permission_grants':
      return unwrapArray(result, 'grants');
    case 'list_project_permission_audit':
      return unwrapArray(result, 'records');
    case 'git_get_branches':
      return unwrapArray(result, 'branches');
    case 'project_ai_model_reasoning_catalog':
      return result?.projection ?? result;
    case 'set_agent_profile_config':
      return 'Agent profile configuration updated successfully';
    case 'reset_agent_profile_config':
      return 'Agent profile configuration reset successfully';
    default:
      return result;
  }
}

/** Maps a frontend `ImageContextData` to the schema `AgentInputAttachment`. */
function encodeImageAttachment(image: any): any {
  const metadata: Record<string, unknown> = {};
  if (image.imagePath) metadata.imagePath = image.imagePath;
  if (image.dataUrl) metadata.dataUrl = image.dataUrl;
  if (image.mimeType) metadata.mimeType = image.mimeType;
  if (image.metadata) metadata.metadata = image.metadata;
  return { kind: 'remote_image', id: image.id, metadata };
}

/** Mirrors `desktop_user_message_metadata`: object-as-is, else wrapped, else empty. */
function encodeUserMetadata(metadata: unknown): Record<string, unknown> {
  if (metadata && typeof metadata === 'object' && !Array.isArray(metadata)) {
    return metadata as Record<string, unknown>;
  }
  if (metadata === undefined || metadata === null) return {};
  return { raw_metadata: metadata };
}

/**
 * Converts the frontend `{requestId, reply: 'once'|'always'|'reject', feedback?}`
 * shape to the schema `RespondPermissionMessage` /
 * `RespondPermissionBatchMessage` wire shape: `{request_id, reply: {reply:
 * 'once'}}` (tagged-enum form).
 */
function encodePermissionResponseBody(body: any): any {
  const { requestId, reply, feedback, ...rest } = body;
  const tagged: Record<string, unknown> = { reply };
  if (reply === 'reject' && feedback) tagged.feedback = feedback;
  return { ...rest, request_id: requestId, reply: tagged };
}

/** Unwraps a `{key: [...]}` response wrapper into the bare array the frontend expects. */
function unwrapArray(result: any, key: string): any {
  if (result && typeof result === 'object' && Array.isArray(result[key])) {
    return result[key];
  }
  return result;
}

interface PendingRequest {
  resolve: (value: any) => void;
  reject: (error: any) => void;
  timeout: NodeJS.Timeout;
  socket: WebSocket;
  timeoutLabel: string;
  outcomeUnknownOnDisconnect: boolean;
}

interface AppServerSyncEventsResponse {
  cursors: unknown[];
  pendingPermissions: Array<Record<string, unknown>>;
  agentSnapshotAvailable: boolean;
  configSnapshotAvailable: boolean;
  externalSourceSnapshotAvailable: boolean;
}

interface SessionOwnerRefreshState {
  generation: number;
  stream: 'agent' | 'app';
  reason:
    | 'agent_snapshot_unavailable'
    | 'event_stream_invalidated'
    | 'side_effect_outcome_unknown';
  resyncMethod: string;
}

interface BufferedPermissionEvent {
  connectionId: string;
  sequence: number;
  event: unknown;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function isPositiveSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0;
}

function isCapabilityDescriptor(value: unknown): boolean {
  if (!isRecord(value) || typeof value.id !== 'string' || value.id.trim() === '') {
    return false;
  }
  const availability = value.availability;
  if (
    !isRecord(availability)
    || (availability.availability !== 'available' && availability.availability !== 'unavailable')
  ) {
    return false;
  }
  if (
    availability.availability === 'unavailable'
    && typeof availability.reason !== 'string'
  ) {
    return false;
  }
  return value.methods === undefined
    || (Array.isArray(value.methods) && value.methods.every(method => typeof method === 'string'));
}

class AppServerProtocolNegotiationError extends Error {}

class SessionOwnerRefreshRequiredError extends AppServerProtocolNegotiationError {}

function validateInitializeResponse(value: unknown): { maxFrameBytes: number } {
  if (!isRecord(value)) {
    throw new AppServerProtocolNegotiationError('Invalid App Server protocol version response');
  }

  const protocolVersion = value.protocolVersion;
  const minimumProtocolVersion = value.minimumProtocolVersion;
  if (
    !isPositiveSafeInteger(protocolVersion)
    || !isPositiveSafeInteger(minimumProtocolVersion)
    || minimumProtocolVersion > protocolVersion
    || APP_SERVER_PROTOCOL_VERSION < minimumProtocolVersion
    || APP_SERVER_PROTOCOL_VERSION > protocolVersion
  ) {
    throw new AppServerProtocolNegotiationError(
      `Incompatible App Server protocol version: client=${APP_SERVER_PROTOCOL_VERSION}, minimum=${String(minimumProtocolVersion)}, current=${String(protocolVersion)}`,
    );
  }

  if (!Array.isArray(value.capabilities) || !value.capabilities.every(isCapabilityDescriptor)) {
    throw new AppServerProtocolNegotiationError('Invalid App Server capabilities response');
  }

  const limits = value.limits;
  if (
    !isRecord(limits)
    || !isPositiveSafeInteger(limits.maxFrameBytes)
    || !isPositiveSafeInteger(limits.eventBufferCapacity)
  ) {
    throw new AppServerProtocolNegotiationError('Invalid App Server transport limits response');
  }
  return { maxFrameBytes: limits.maxFrameBytes as number };
}

function validateSyncEventsResponse(value: unknown): AppServerSyncEventsResponse {
  if (
    !isRecord(value)
    || !Array.isArray(value.cursors)
    || !Array.isArray(value.pendingPermissions)
    || !value.pendingPermissions.every(isRecord)
    || typeof value.agentSnapshotAvailable !== 'boolean'
    || typeof value.configSnapshotAvailable !== 'boolean'
    || typeof value.externalSourceSnapshotAvailable !== 'boolean'
  ) {
    throw new AppServerProtocolNegotiationError('Invalid App Server event synchronization response');
  }
  return value as unknown as AppServerSyncEventsResponse;
}

function isPermanentInitializeFailure(error: unknown): boolean {
  if (error instanceof AppServerProtocolNegotiationError) {
    return true;
  }

  const data = error instanceof Error
    ? (error as Error & { data?: unknown }).data
    : undefined;
  return isRecord(data)
    && data.capability === APP_SERVER_INITIALIZE_CAPABILITY
    && data.retryable === false;
}

export function webSocketResponseError(value: unknown): Error {
  const record = value && typeof value === 'object'
    ? value as { code?: unknown; message?: unknown; data?: unknown }
    : undefined;
  const message = typeof record?.message === 'string'
    ? record.message
    : String(value);
  const error = new Error(message) as Error & { code?: unknown; data?: unknown };
  error.code = record?.code;
  error.data = record?.data;
  return error;
}

export interface DecodedWsNotification {
  event: string;
  payload: unknown;
}

/** Project one app-server JSON-RPC notification into the frontend event bus. */
export function decodeWsNotification(message: any): DecodedWsNotification | null {
  if (message?.method === 'agent/event' && message.params?.event) {
    const projected = projectAgenticEventEnvelope(message.params.event);
    if (!projected) return null;
    return {
      event: projected.eventName,
      payload: projected.payload,
    };
  }
  if (message?.method === 'agent/permissionEvent' && message.params?.event) {
    return {
      event: 'permission://event',
      payload: message.params.event,
    };
  }
  if (message?.method === 'config/event' && message.params?.event) {
    return {
      event: 'config://updated',
      payload: message.params.event as ConfigUpdate,
    };
  }
  if (message?.method === 'app/eventStreamState' && message.params) {
    return {
      event: 'app-server://event-stream-state',
      payload: message.params,
    };
  }
  return null;
}

export class WebSocketTransportAdapter implements ITransportAdapter {
  private ws: WebSocket | null = null;
  private url: string;
  private eventListeners: Map<string, Set<(data: any) => void>> = new Map();
  private pendingRequests: Map<string, PendingRequest> = new Map();
  private messageIdCounter = 0;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  private protocolReady = false;
  private connectionGeneration = 0;
  private activeGeneration = 0;
  private sessionOwnerRefreshRequired: SessionOwnerRefreshState | null = null;
  private maxFrameBytes: number | null = null;
  private pendingPermissionRequests = new Map<string, Record<string, unknown>>();
  private permissionSyncPromise: Promise<AppServerSyncEventsResponse> | null = null;
  private permissionSyncSocket: WebSocket | null = null;
  private permissionSyncDirty = false;
  private bufferedPermissionEvents: BufferedPermissionEvent[] = [];
  // Held so `disconnect()` can cancel a reconnect that is already scheduled; without it a
  // pending timer reopens the socket after an explicit teardown.
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  
  constructor(url?: string) {
    
    this.url = url || import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws';
  }
  
   
  async connect(): Promise<void> {
    this.assertSessionOwnerRefreshNotRequired();
    if (this.protocolReady && this.ws?.readyState === WebSocket.OPEN) {
      return;
    }
    if (this.connectPromise) {
      return this.connectPromise;
    }

    const attempt = this.connectAndInitialize();
    this.connectPromise = attempt;
    try {
      await attempt;
    } finally {
      if (this.connectPromise === attempt) {
        this.connectPromise = null;
      }
    }
  }

  private async connectAndInitialize(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        log.info('Connecting', { url: this.url });
        const ws = new WebSocket(this.url);
        this.ws = ws;
        this.protocolReady = false;
        this.maxFrameBytes = null;
        let settled = false;
        let reconnectOnClose = true;

        ws.onopen = async () => {
          this.setupMessageHandler(ws);
          try {
            const initialized = await this.sendJsonRpcRequest<unknown>(APP_SERVER_INITIALIZE_METHOD, {
              protocolVersion: APP_SERVER_PROTOCOL_VERSION,
              client: {
                name: 'bitfun-web-ui',
                version: getVersionInfo().version,
              },
            }, APP_SERVER_INITIALIZE_METHOD);
            const negotiated = validateInitializeResponse(initialized);
            this.maxFrameBytes = negotiated.maxFrameBytes;
            if (settled) return;
            const generation = this.connectionGeneration + 1;
            this.activeGeneration = generation;
            const synchronized = await this.synchronizeEvents();
            if (settled) return;
            this.connectionGeneration = generation;
            if (generation > 1 && !synchronized.agentSnapshotAvailable) {
              this.markSessionOwnerRefreshRequired({
                generation,
                stream: 'agent',
                reason: 'agent_snapshot_unavailable',
                resyncMethod: 'session/sync',
              });
              throw new SessionOwnerRefreshRequiredError(SESSION_OWNER_REFRESH_ERROR);
            }
            this.protocolReady = true;
            this.reconnectAttempts = 0;
            settled = true;
            log.info('Connected successfully');
            resolve();
          } catch (error) {
            if (settled) return;
            settled = true;
            this.protocolReady = false;
            reconnectOnClose = !isPermanentInitializeFailure(error);
            reject(error);
            ws.close();
          }
        };
        
        ws.onerror = (error) => {
          log.error('Connection error', error);
          if (!settled) {
            settled = true;
            reject(new Error('WebSocket connection failed'));
          }
        };
        
        ws.onclose = () => {
          log.info('Connection closed');
          this.resetPermissionSyncForSocket(ws);
          this.rejectPendingRequestsForSocket(ws);
          if (this.ws !== ws) {
            return;
          }
          this.protocolReady = false;
          if (!settled) {
            settled = true;
            reject(new Error('WebSocket connection closed before protocol initialization'));
          }
          if (reconnectOnClose) {
            this.handleDisconnect();
          }
        };
      } catch (error) {
        log.error('Failed to create WebSocket', error);
        reject(error);
      }
    });
  }
  
   
  private connectPromise: Promise<void> | null = null;

  private async ensureConnected(): Promise<void> {
    if (this.protocolReady && this.ws?.readyState === WebSocket.OPEN) {
      await this.permissionSyncPromise;
      this.assertBusinessReady();
      return;
    }
    await this.connect();
    await this.permissionSyncPromise;
    this.assertBusinessReady();
  }

  private assertBusinessReady(): void {
    this.assertSessionOwnerRefreshNotRequired();
  }

  private assertSessionOwnerRefreshNotRequired(): void {
    if (this.sessionOwnerRefreshRequired) {
      throw new SessionOwnerRefreshRequiredError(SESSION_OWNER_REFRESH_ERROR);
    }
  }

  private async synchronizeEvents(
    streams: Array<'agent' | 'permission' | 'config'> = ['agent', 'permission', 'config'],
  ): Promise<AppServerSyncEventsResponse> {
    if (!streams.includes('permission')) {
      return this.requestEventSynchronization(streams);
    }
    const socket = this.ws;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not open');
    }
    if (this.permissionSyncPromise && this.permissionSyncSocket === socket) {
      this.permissionSyncDirty = true;
      return this.permissionSyncPromise;
    }

    const attempt = (async () => {
      let response: AppServerSyncEventsResponse | null = null;
      let requestedStreams = streams;
      do {
        this.permissionSyncDirty = false;
        response = await this.requestEventSynchronization(requestedStreams);
        this.reconcilePermissionSnapshotAtWatermark(response);
        requestedStreams = ['permission'];
      } while (this.permissionSyncDirty);
      return response;
    })();
    this.permissionSyncPromise = attempt;
    this.permissionSyncSocket = socket;
    try {
      return await attempt;
    } finally {
      if (this.permissionSyncPromise === attempt) {
        this.permissionSyncPromise = null;
        this.permissionSyncSocket = null;
      }
    }
  }

  private resetPermissionSyncForSocket(socket: WebSocket): void {
    if (this.permissionSyncSocket !== socket) {
      return;
    }
    this.permissionSyncPromise = null;
    this.permissionSyncSocket = null;
    this.permissionSyncDirty = false;
    this.bufferedPermissionEvents = [];
  }

  private rejectPendingRequestsForSocket(socket: WebSocket): void {
    let sideEffectOutcomeUnknown = false;
    for (const [requestId, pending] of this.pendingRequests) {
      if (pending.socket !== socket) {
        continue;
      }
      clearTimeout(pending.timeout);
      this.pendingRequests.delete(requestId);
      const message = pending.outcomeUnknownOnDisconnect
        ? `outcome_unknown: ${pending.timeoutLabel} may have been applied before the App Server connection closed; reload authoritative state before retrying`
        : `WebSocket disconnected before response: ${pending.timeoutLabel}`;
      sideEffectOutcomeUnknown ||= pending.outcomeUnknownOnDisconnect;
      pending.reject(new Error(message));
    }
    if (sideEffectOutcomeUnknown && this.ws === socket) {
      this.markSideEffectOutcomeUnknown();
    }
  }

  private async requestEventSynchronization(
    streams: Array<'agent' | 'permission' | 'config'>,
  ): Promise<AppServerSyncEventsResponse> {
    return validateSyncEventsResponse(
      await this.sendJsonRpcRequest<unknown>(APP_SERVER_SYNC_EVENTS_METHOD, {
        streams,
      }, APP_SERVER_SYNC_EVENTS_METHOD),
    );
  }

  private reconcilePermissionSnapshotAtWatermark(
    response: AppServerSyncEventsResponse,
  ): void {
    const cursor = response.cursors.find(value => (
      isRecord(value)
      && value.stream === 'permission'
      && typeof value.connectionId === 'string'
      && value.connectionId.length > 0
      && typeof value.sequence === 'number'
      && Number.isSafeInteger(value.sequence)
      && value.sequence >= 0
    )) as Record<string, unknown> | undefined;
    if (!cursor) {
      throw new AppServerProtocolNegotiationError(
        'Permission synchronization response is missing its authoritative cursor',
      );
    }

    const connectionId = cursor.connectionId as string;
    const watermark = cursor.sequence as number;
    this.reconcilePermissionSnapshot(response.pendingPermissions);
    const buffered = this.bufferedPermissionEvents;
    this.bufferedPermissionEvents = [];
    for (const entry of buffered) {
      if (entry.connectionId !== connectionId) {
        throw new AppServerProtocolNegotiationError(
          'Permission event belongs to a different App Server connection',
        );
      }
      if (entry.sequence > watermark) {
        this.handlePermissionEvent(entry.event);
      }
    }
  }

  private bufferPermissionEvent(cursor: unknown, event: unknown): boolean {
    if (
      !isRecord(cursor)
      || cursor.stream !== 'permission'
      || typeof cursor.connectionId !== 'string'
      || cursor.connectionId.length === 0
      || typeof cursor.sequence !== 'number'
      || !Number.isSafeInteger(cursor.sequence)
      || cursor.sequence < 0
    ) {
      return false;
    }
    this.bufferedPermissionEvents.push({
      connectionId: cursor.connectionId,
      sequence: cursor.sequence,
      event,
    });
    return true;
  }

  private resynchronizePermissions(): void {
    const socket = this.ws;
    void this.synchronizeEvents(['permission']).catch(error => {
      log.error('Failed to resynchronize permission events', error);
      if (this.ws === socket && socket?.readyState === WebSocket.OPEN) {
        socket?.close();
      }
    });
  }

  private reconcilePermissionSnapshot(
    pendingPermissions: Array<Record<string, unknown>>,
  ): void {
    const next = new Map<string, Record<string, unknown>>();
    for (const request of pendingPermissions) {
      if (typeof request.requestId === 'string' && request.requestId) {
        next.set(request.requestId, request);
      }
    }

    for (const requestId of this.pendingPermissionRequests.keys()) {
      if (next.has(requestId)) continue;
      this.emitEvent('permission://event', {
        event: 'cancelled',
        requestId,
        reason: 'Permission request is absent from the authoritative reconnect snapshot',
      });
    }

    for (const [requestId, request] of next) {
      const alreadyPending = this.pendingPermissionRequests.has(requestId);
      if (!alreadyPending) {
        this.emitEvent('permission://event', { event: 'asked', request });
      }
    }
    this.pendingPermissionRequests = next;
  }

  private handlePermissionEvent(value: unknown): boolean {
    if (!isRecord(value) || typeof value.event !== 'string') return false;
    if (value.event === 'asked') {
      if (!isRecord(value.request) || typeof value.request.requestId !== 'string') return false;
      const requestId = value.request.requestId;
      const alreadyPending = this.pendingPermissionRequests.has(requestId);
      this.pendingPermissionRequests.set(requestId, value.request);
      if (!alreadyPending) this.emitEvent('permission://event', value);
      return true;
    }
    if (
      (value.event === 'replied' || value.event === 'cancelled')
      && typeof value.requestId === 'string'
    ) {
      const requestId = value.requestId;
      if (!this.pendingPermissionRequests.has(requestId)) return true;
      this.pendingPermissionRequests.delete(requestId);
      this.emitEvent('permission://event', value);
      return true;
    }
    return false;
  }

  private markSessionOwnerRefreshRequired(state: SessionOwnerRefreshState): void {
    if (
      this.sessionOwnerRefreshRequired?.generation === state.generation
      && this.sessionOwnerRefreshRequired.reason === state.reason
    ) {
      return;
    }
    this.sessionOwnerRefreshRequired = state;
    this.emitEvent(SESSION_OWNER_REFRESH_EVENT, state);
  }

  private markSideEffectOutcomeUnknown(): void {
    if (this.sessionOwnerRefreshRequired) {
      return;
    }
    this.markSessionOwnerRefreshRequired({
      generation: this.activeGeneration,
      stream: 'app',
      reason: 'side_effect_outcome_unknown',
      resyncMethod: 'reload',
    });
  }

  private closeForSessionOwnerRefresh(state: SessionOwnerRefreshState): void {
    this.markSessionOwnerRefreshRequired(state);
    this.protocolReady = false;
    const error = new SessionOwnerRefreshRequiredError(SESSION_OWNER_REFRESH_ERROR);
    this.pendingRequests.forEach(pending => {
      clearTimeout(pending.timeout);
      pending.reject(error);
    });
    this.pendingRequests.clear();
    this.ws?.close();
  }

  private emitEvent(eventName: string, payload: unknown): void {
    const listeners = this.eventListeners.get(eventName);
    if (!listeners || listeners.size === 0) return;
    listeners.forEach(callback => {
      try {
        callback(payload);
      } catch (error) {
        log.error('Error in event listener', { event: eventName, error });
      }
    });
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private handleDisconnect(): void {
    if (this.sessionOwnerRefreshRequired) {
      log.error('Automatic reconnect blocked until the Web page reloads Session owner state');
      return;
    }
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      const delay = this.reconnectDelay * this.reconnectAttempts;
      
      log.info('Reconnecting', { delay, attempt: this.reconnectAttempts, maxAttempts: this.maxReconnectAttempts });

      this.clearReconnectTimer();
      this.reconnectTimer = setTimeout(() => {
        this.reconnectTimer = null;
        this.connect().catch(error => {
          log.error('Reconnection failed', error);
        });
      }, delay);
    } else {
      log.error('Max reconnection attempts reached');
      
      this.pendingRequests.forEach((pending) => {
        clearTimeout(pending.timeout);
        pending.reject(new Error('WebSocket disconnected'));
      });
      this.pendingRequests.clear();
    }
  }
  
   
  private setupMessageHandler(ws: WebSocket): void {
    ws.onmessage = (event) => {
      if (this.ws !== ws) {
        return;
      }
      try {
        const message = JSON.parse(event.data);

        // JSON-RPC 2.0 response: carries `id` matching a pending request.
        // Shape: `{ jsonrpc: "2.0", id, result | error }`.
        if (message.id !== undefined && this.pendingRequests.has(message.id)) {
          const pending = this.pendingRequests.get(message.id)!;
          if (pending.socket !== ws) {
            return;
          }
          clearTimeout(pending.timeout);

          if (message.error) {
            pending.reject(webSocketResponseError(message.error));
          } else {
            pending.resolve(message.result);
          }

          this.pendingRequests.delete(message.id);
          return;
        }

        // JSON-RPC 2.0 notification: `{ jsonrpc: "2.0", method, params }`.
        if (message.method === 'agent/permissionEvent') {
          if (this.permissionSyncPromise) {
            if (!this.bufferPermissionEvent(message.params?.cursor, message.params?.event)) {
              ws.close();
            }
            return;
          }
          this.handlePermissionEvent(message.params?.event);
          return;
        }
        if (
          message.method === 'app/eventStreamState'
          && message.params?.stream === 'agent'
          && message.params?.resync?.snapshotAvailable === false
        ) {
          this.closeForSessionOwnerRefresh({
            generation: this.activeGeneration,
            stream: 'agent',
            reason: 'event_stream_invalidated',
            resyncMethod: typeof message.params.resync.method === 'string'
              ? message.params.resync.method
              : 'session/sync',
          });
        } else if (
          message.method === 'app/eventStreamState'
          && message.params?.stream === 'permission'
          && (message.params?.state === 'lagged' || message.params?.state === 'invalidated')
          && message.params?.resync?.method === APP_SERVER_SYNC_EVENTS_METHOD
          && message.params?.resync?.snapshotAvailable === true
        ) {
          this.resynchronizePermissions();
        } else if (
          message.method === 'app/eventStreamState'
          && message.params?.stream === 'permission'
          && (message.params?.state === 'closed' || message.params?.resync?.snapshotAvailable !== true)
        ) {
          ws.close();
        }
        const notification = decodeWsNotification(message);
        if (notification) {
          this.emitEvent(notification.event, notification.payload);
          return;
        }

        // Legacy `agentic://<type>` pass-through (kept for any notification the
        // server still sends with a bare `type` field in the old shape).
        const mappedEventName = this.mapLegacyTypeToEventName(message.type);
        if (mappedEventName) {
          const listeners = this.eventListeners.get(mappedEventName);
          if (listeners && listeners.size > 0) {
            listeners.forEach(callback => {
              try {
                callback(message);
              } catch (error) {
                log.error('Error in WebSocket type-mapped event listener', {
                  event: mappedEventName,
                  rawType: message.type,
                  error,
                });
              }
            });
          }
        }
      } catch (error) {
        log.error('Failed to parse message', { data: event.data, error });
      }
    };
  }

  private mapLegacyTypeToEventName(type: unknown): string | null {
    if (typeof type !== 'string' || !type.trim()) {
      return null;
    }

    return `agentic://${type.trim()}`;
  }
  

  async request<T>(action: string, params?: any): Promise<T>;
  async request<K extends AgentCommandKey>(
    action: K,
    params: (typeof AGENT_COMMAND_SCHEMA)[K] extends { request: infer R } ? R : undefined,
  ): Promise<
    (typeof AGENT_COMMAND_SCHEMA)[K] extends { response: infer S } ? S : unknown
  >;
  async request<T>(action: string, params?: any): Promise<T> {
    await this.ensureConnected();

    // Translate the frontend snake_case command to the app-server JSON-RPC
    // method so agent-kernel requests reach the app-server surface in web mode.
    const method = resolveWsMethod(action);
    // The service layer calls `api.invoke('cmd', { request: {...} })` (Tauri
    // convention). ACP `on_receive_request` deserializes the bare body, so
    // unwrap the `{request}` envelope here.
    const envelope = (params && typeof params === 'object'
      && 'request' in params
      && Object.keys(params).length === 1)
      ? params.request ?? {}
      : params ?? {};
    // Encode the frontend body into the app-server schema wire shape (field
    // names, tagged enums, attachment projection). Mirrors the conversions the
    // desktop host does in Rust (`agentic_api.rs`).
    const body = encodeRequestBody(action, envelope);

    const command = (AGENT_COMMAND_SCHEMA as Record<string, AgentCommandEntry>)[action];
    return this.sendJsonRpcRequest(
      method,
      body,
      action,
      value => decodeResponseBody(action, value),
      command?.sideEffecting === true,
    );
  }

  private sendJsonRpcRequest<T>(
    method: string,
    params: unknown,
    timeoutLabel: string,
    project: (value: any) => T = value => value as T,
    outcomeUnknownOnDisconnect = false,
  ): Promise<T> {
    return new Promise((resolve, reject) => {
      const ws = this.ws;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        reject(new Error('WebSocket is not open'));
        return;
      }

      const messageId = `msg_${Date.now()}_${++this.messageIdCounter}`;
      let payload: string;
      try {
        payload = JSON.stringify({
          jsonrpc: '2.0',
          id: messageId,
          method,
          params,
        });
      } catch (error) {
        reject(error);
        return;
      }
      const payloadBytes = new TextEncoder().encode(payload).byteLength;
      if (this.maxFrameBytes !== null && payloadBytes > this.maxFrameBytes) {
        const error = new Error(
          `Request ${timeoutLabel} is ${payloadBytes} bytes and exceeds negotiated App Server frame limit ${this.maxFrameBytes} bytes`,
        ) as Error & { code?: string; maxFrameBytes?: number; payloadBytes?: number };
        error.code = 'payload_too_large';
        error.maxFrameBytes = this.maxFrameBytes;
        error.payloadBytes = payloadBytes;
        reject(error);
        return;
      }
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(messageId);
        const message = outcomeUnknownOnDisconnect
          ? `outcome_unknown: ${timeoutLabel} may have been applied before its App Server response timed out; reload authoritative state before retrying`
          : `Request timeout: ${timeoutLabel}`;
        reject(new Error(message));
        if (outcomeUnknownOnDisconnect && this.ws === ws) {
          this.markSideEffectOutcomeUnknown();
          this.protocolReady = false;
          ws.close();
        }
      }, 30000);

      this.pendingRequests.set(messageId, {
        resolve: (value: any) => resolve(project(value)),
        reject,
        timeout,
        socket: ws,
        timeoutLabel,
        outcomeUnknownOnDisconnect,
      });

      try {
        ws.send(payload);
      } catch (error) {
        clearTimeout(timeout);
        this.pendingRequests.delete(messageId);
        reject(error);
      }
    });
  }
  
   
  listen<T>(event: string, callback: (data: T) => void): () => void {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, new Set());
    }
    
    const listeners = this.eventListeners.get(event)!;
    listeners.add(callback);
    if (event === SESSION_OWNER_REFRESH_EVENT && this.sessionOwnerRefreshRequired) {
      try {
        callback(this.sessionOwnerRefreshRequired as T);
      } catch (error) {
        log.error('Error replaying durable event state to listener', { event, error });
      }
    }
    
    
    return () => {
      const listeners = this.eventListeners.get(event);
      if (listeners) {
        listeners.delete(callback);
        if (listeners.size === 0) {
          this.eventListeners.delete(event);
        }
      }
    };
  }
  
   
  async disconnect(): Promise<void> {

    // Before anything else, so a reconnect that is already queued cannot fire after teardown
    // and reopen the socket we are about to close.
    this.clearReconnectTimer();

    this.pendingRequests.forEach((pending) => {
      clearTimeout(pending.timeout);
      pending.reject(new Error('WebSocket manually disconnected'));
    });
    this.pendingRequests.clear();
    
    
    this.eventListeners.clear();
    
    
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    
    
    this.reconnectAttempts = this.maxReconnectAttempts;
    this.protocolReady = false;
    this.connectionGeneration = 0;
    this.activeGeneration = 0;
    this.sessionOwnerRefreshRequired = null;
    this.maxFrameBytes = null;
    this.pendingPermissionRequests.clear();
    this.permissionSyncPromise = null;
    this.permissionSyncSocket = null;
    this.permissionSyncDirty = false;
    this.bufferedPermissionEvents = [];
    this.connectPromise = null;
  }
  
   
  isConnected(): boolean {
    return this.protocolReady
      && !this.sessionOwnerRefreshRequired
      && this.ws !== null
      && this.ws.readyState === WebSocket.OPEN;
  }
}
