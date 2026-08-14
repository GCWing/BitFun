import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  AGENT_COMMAND_SCHEMA,
  decodeWsNotification,
  decodeResponseBody,
  encodeRequestBody,
  resolveWsMethod,
  SESSION_OWNER_REFRESH_EVENT,
  WebSocketTransportAdapter,
} from './websocket-adapter';
import type {
  SubmitDialogTurnBody,
  SubmitDialogTurnResponse,
} from '@/generated/api';

class ProtocolMockWebSocket {
  static readonly OPEN = 1;

  readyState = 0;
  sent: Array<Record<string, unknown>> = [];
  onopen: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;

  constructor(readonly url: string) {}

  open(): void {
    this.readyState = ProtocolMockWebSocket.OPEN;
    this.onopen?.({} as Event);
  }

  receive(message: Record<string, unknown>): void {
    this.onmessage?.({ data: JSON.stringify(message) } as MessageEvent);
  }

  send(payload: string): void {
    this.sent.push(JSON.parse(payload) as Record<string, unknown>);
  }

  close(): void {
    this.readyState = 3;
    this.onclose?.({} as CloseEvent);
  }
}

function installProtocolWebSocket(): ProtocolMockWebSocket[] {
  const sockets: ProtocolMockWebSocket[] = [];
  class TestWebSocket extends ProtocolMockWebSocket {
    constructor(url: string) {
      super(url);
      sockets.push(this);
    }
  }
  vi.stubGlobal('WebSocket', TestWebSocket);
  return sockets;
}

function validInitializeResult(maxFrameBytes = 256 * 1024): Record<string, unknown> {
  return {
    protocolVersion: 4,
    minimumProtocolVersion: 4,
    server: { name: 'bitfun-app-server', version: '0.2.18' },
    capabilities: [
      {
        id: 'agent',
        availability: { availability: 'available' },
        methods: ['agent/listSessions'],
      },
      {
        id: 'eventSync',
        availability: { availability: 'available' },
        methods: ['app/syncEvents', 'app/eventStreamState'],
      },
    ],
    limits: { maxFrameBytes, eventBufferCapacity: 1024 },
  };
}

function validSyncEventsResult(
  overrides: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    cursors: [
      { connectionId: 'app-server-1', stream: 'agent', sequence: 0 },
      { connectionId: 'app-server-1', stream: 'permission', sequence: 0 },
      { connectionId: 'app-server-1', stream: 'config', sequence: 0 },
    ],
    pendingPermissions: [],
    agentSnapshotAvailable: false,
    configSnapshotAvailable: false,
    externalSourceSnapshotAvailable: false,
    ...overrides,
  };
}

function permissionRequest(requestId: string): Record<string, unknown> {
  return {
    requestId,
    roundId: 'round-1',
    order: 0,
    projectId: 'project-1',
    sessionId: 'session-1',
    agentId: 'agentic',
    action: 'filesystem.read',
    resources: [`/workspace/${requestId}.txt`],
    source: { kind: 'tool_call', identity: `tool-${requestId}` },
  };
}

describe('resolveWsMethod', () => {
  it('maps every app-server agent command to its agent/* surface method', () => {
    // The snake_case Tauri command names the service-API call sites use are
    // resolved to the schema `group/verb` method names via the typed
    // `AGENT_COMMAND_SCHEMA` table (web transport only -- desktop/CLI call Tauri
    // commands directly). If one is added/removed on the Rust side, update the
    // table and this case in lockstep.
    expect(resolveWsMethod('create_session')).toBe('agent/createSession');
    expect(resolveWsMethod('list_sessions')).toBe('agent/listSessions');
    expect(resolveWsMethod('delete_session')).toBe('agent/deleteSession');
    expect(resolveWsMethod('fork_session')).toBe('session/forkAtTurn');
    expect(resolveWsMethod('archive_session')).toBe('session/setArchived');
    expect(resolveWsMethod('unarchive_session')).toBe('session/setArchived');
    // `start_dialog_turn` -> `agent/submitDialogTurn` (NOT `agent/submitTurn`):
    // the dialog-turn body carries `agentType`/`workspacePath`/`policy`. Mapping
    // it to `submitTurn` would silently drop `agentType` and omit the required
    // `policy` -- see `agentic_api.rs::start_dialog_turn` for the desktop path.
    expect(resolveWsMethod('start_dialog_turn')).toBe('agent/submitDialogTurn');
    expect(resolveWsMethod('cancel_dialog_turn')).toBe('agent/cancelTurn');
    // Permission surface: reply/list/grants map to the app-server permission
    // methods. `subscribe_permission_requests` is satisfied by the adapter's
    // typed `agent/permissionEvent` + `app/syncEvents` projection; this command
    // mapping retains the current pending-list compatibility call.
    expect(resolveWsMethod('respond_permission')).toBe('agent/respondPermission');
    expect(resolveWsMethod('respond_permission_batch')).toBe(
      'agent/respondPermissionBatch'
    );
    expect(resolveWsMethod('list_pending_permission_requests')).toBe(
      'agent/listPendingPermissionRequests'
    );
    expect(resolveWsMethod('subscribe_permission_requests')).toBe(
      'agent/listPendingPermissionRequests'
    );
    expect(resolveWsMethod('list_project_permission_grants')).toBe(
      'agent/listProjectPermissionGrants'
    );
    expect(resolveWsMethod('remove_project_permission_grant')).toBe(
      'agent/removeProjectPermissionGrant'
    );
    expect(resolveWsMethod('clear_project_permission_grants')).toBe(
      'agent/clearProjectPermissionGrants'
    );
  });

  it('passes non-agent commands through unchanged', () => {
    // ping and external_sources are handled by the Server Host dispatch
    // directly; commands the server does not expose over WS also flow through
    // unchanged so they surface as a clean "unknown command" error rather than
    // a silent rename. `get_project_permission_rules`/`save_project_permission_rules`
    // are desktop-host-managed permission rule files (no AgentRuntime SDK op),
    // so they pass through and fail as "unknown command" in web mode until a
    // later track adds a host-services dispatch.
    expect(resolveWsMethod('ping')).toBe('ping');
    expect(resolveWsMethod('get_external_source_snapshot')).toBe(
      'get_external_source_snapshot'
    );
    expect(resolveWsMethod('get_project_permission_rules')).toBe(
      'get_project_permission_rules'
    );
    expect(resolveWsMethod('some_future_command')).toBe('some_future_command');
    expect(resolveWsMethod('')).toBe('');
  });

  it('maps the git service commands to their git/* surface methods', () => {
    // Read-only git operations exposed by the app-server surface (option C).
    // Write operations and the remote (SSH) path arrive later; the Server Host
    // has no SSH manager, so remote git paths surface as
    // `host_capability_unavailable` (the `external_sources` precedent).
    expect(resolveWsMethod('git_is_repository')).toBe('git/isRepository');
    expect(resolveWsMethod('git_get_status')).toBe('git/getStatus');
    expect(resolveWsMethod('git_get_branches')).toBe('git/getBranches');
  });

  it('maps the config service commands to their config/* surface methods', () => {
    // Read-only config operations: agent-profile canonicalizer + AI model
    // configs + single/batched config-path reads. `get_config`/`get_configs`
    // carry the not-found -> undefined contract the frontend `ConfigAPI`
    // depends on (the app-server surfaces the `BitFunError::NotFound` Display
    // text as the JSON-RPC `message`). `get_skill_configs` still arrives
    // later (workspace dependency).
    expect(resolveWsMethod('get_agent_profile_configs')).toBe(
      'config/getAgentProfileConfigs'
    );
    expect(resolveWsMethod('get_agent_profile_config')).toBe(
      'config/getAgentProfileConfig'
    );
    expect(resolveWsMethod('get_model_configs')).toBe('config/getModelConfigs');
    expect(resolveWsMethod('project_ai_model_reasoning_catalog')).toBe(
      'model/projectReasoningCatalog',
    );
    expect(resolveWsMethod('get_config')).toBe('config/getConfig');
    expect(resolveWsMethod('get_configs')).toBe('config/getConfigs');
    expect(resolveWsMethod('set_agent_profile_config')).toBe(
      'config/setAgentProfileConfig'
    );
    expect(resolveWsMethod('reset_agent_profile_config')).toBe(
      'config/resetAgentProfileConfig'
    );
    expect(resolveWsMethod('set_config')).toBe('config/setConfig');
    expect(resolveWsMethod('save_cloud_speech_config')).toBe(
      'config/saveCloudSpeechConfig'
    );
    expect(resolveWsMethod('validate_config')).toBe('config/validateConfig');
    expect(resolveWsMethod('i18n_get_current_language')).toBe(
      'i18n/getCurrentLanguage'
    );
    expect(resolveWsMethod('i18n_set_language')).toBe('i18n/setLanguage');
    expect(resolveWsMethod('i18n_get_config')).toBe('i18n/getConfig');
    expect(resolveWsMethod('i18n_set_config')).toBe('i18n/setConfig');
    expect(resolveWsMethod('i18n_get_supported_languages')).toBe(
      'i18n/getSupportedLanguages'
    );
  });

  it('pins the typed command schema (Step 1: generated types wired into the method table)', () => {
    // Compile-time: the start_dialog_turn request/response slots are bound to
    // the generated schema types. These locals are intentionally unused at
    // runtime; they exist only so `tsc` fails if the generated barrel or the
    // schema slot type drifts.
    const _body: SubmitDialogTurnBody = {
      sessionId: '',
      message: '',
      agentType: '',
    };
    const _resp: SubmitDialogTurnResponse = { status: 'started', sessionId: '', turnId: '' };

    // Runtime sanity: the schema entry carries the method string and the table
    // covers the schema methods (key count is stable; ordering is not pinned
    // because the table is a plain object). Track B Batch 1 added config write +
    // i18n and the P0 Session/Config control plane. Atomic cloud-speech save
    // config validation, and live reasoning projection raise the count to 34.
    expect(AGENT_COMMAND_SCHEMA.start_dialog_turn.method).toBe(
      'agent/submitDialogTurn'
    );
    expect(Object.keys(AGENT_COMMAND_SCHEMA).length).toBe(34);

    // Touch the locals so noUnusedLocals does not flag them under vitest's
    // transformed build (tsc --noEmit is the real gate; this is belt-and-suspenders).
    expect(_body.message).toBe('');
    expect(_resp.status).toBe('started');
  });
});

describe('encodeRequestBody', () => {
  it('renames start_dialog_turn fields to the schema wire shape', () => {
    const frontend = {
      sessionId: 's1',
      userInput: 'hello',
      originalUserInput: 'hello!',
      agentType: 'coder',
      workspacePath: '/repo',
      imageContexts: [{ id: 'img1', imagePath: '/tmp/a.png', mimeType: 'image/png' }],
      userMessageMetadata: { hint: 'fast' },
    };
    const encoded = encodeRequestBody('start_dialog_turn', frontend);
    expect(encoded.sessionId).toBe('s1');
    expect(encoded.message).toBe('hello');
    expect(encoded.originalMessage).toBe('hello!');
    expect(encoded.agentType).toBe('coder');
    expect(encoded.workspacePath).toBe('/repo');
    expect(encoded.attachments).toEqual([
      { kind: 'remote_image', id: 'img1', metadata: { imagePath: '/tmp/a.png', mimeType: 'image/png' } },
    ]);
    expect(encoded.metadata).toEqual({ hint: 'fast' });
    // Original field names must not leak through.
    expect(encoded.userInput).toBeUndefined();
    expect(encoded.imageContexts).toBeUndefined();
    expect(encoded.userMessageMetadata).toBeUndefined();
  });

  it('omits optional start_dialog_turn fields when absent', () => {
    const encoded = encodeRequestBody('start_dialog_turn', {
      sessionId: 's1',
      userInput: 'hi',
      agentType: 'coder',
    });
    expect(encoded.message).toBe('hi');
    expect(encoded.originalMessage).toBeUndefined();
    expect(encoded.attachments).toBeUndefined();
    expect(encoded.metadata).toBeUndefined();
  });

  it('wraps non-object userMessageMetadata in raw_metadata', () => {
    const encoded = encodeRequestBody('start_dialog_turn', {
      sessionId: 's1',
      userInput: 'hi',
      agentType: 'coder',
      userMessageMetadata: 'plain string',
    });
    expect(encoded.metadata).toEqual({ raw_metadata: 'plain string' });
  });

  it('converts respond_permission reply string to tagged-enum shape', () => {
    const encoded = encodeRequestBody('respond_permission', {
      requestId: 'r1',
      reply: 'once',
    });
    expect(encoded.request_id).toBe('r1');
    expect(encoded.reply).toEqual({ reply: 'once' });
    expect(encoded.requestId).toBeUndefined();
  });

  it('embeds feedback into reject reply', () => {
    const encoded = encodeRequestBody('respond_permission_batch', {
      requestId: 'r2',
      reply: 'reject',
      feedback: 'no thanks',
    });
    expect(encoded.request_id).toBe('r2');
    expect(encoded.reply).toEqual({ reply: 'reject', feedback: 'no thanks' });
  });

  it('omits feedback when reply is not reject', () => {
    const encoded = encodeRequestBody('respond_permission', {
      requestId: 'r3',
      reply: 'always',
      feedback: 'should be dropped',
    });
    expect(encoded.reply).toEqual({ reply: 'always' });
  });

  it('encodes fork-at-turn and archive state into Session wire DTOs', () => {
    expect(encodeRequestBody('fork_session', {
      workspace_path: '/repo',
      source_session_id: 's1',
      source_turn_id: 't2',
      remote_connection_id: 'remote-1',
    })).toEqual({
      workspacePath: '/repo',
      sourceSessionId: 's1',
      sourceTurnId: 't2',
      remoteConnectionId: 'remote-1',
    });

    expect(encodeRequestBody('archive_session', {
      workspace_path: '/repo',
      session_id: 's1',
    })).toEqual({
      workspacePath: '/repo',
      sessionId: 's1',
      archived: true,
    });
    expect(encodeRequestBody('unarchive_session', {
      workspace_path: '/repo',
      session_id: 's1',
    })).toEqual({
      workspacePath: '/repo',
      sessionId: 's1',
      archived: false,
    });
  });

  it('passes unknown actions through unchanged', () => {
    const body = { foo: 'bar' };
    expect(encodeRequestBody('some_unknown_action', body)).toBe(body);
    expect(encodeRequestBody('list_sessions', body)).toBe(body);
  });

  it('preserves the desktop success-string contract for profile mutations', () => {
    expect(decodeResponseBody('set_agent_profile_config', { profile_id: 'agentic' }))
      .toBe('Agent profile configuration updated successfully');
    expect(decodeResponseBody('reset_agent_profile_config', { profile_id: 'agentic' }))
      .toBe('Agent profile configuration reset successfully');
  });
});

describe('decodeWsNotification', () => {
  it('projects a real v4 agent/event envelope to the existing agentic event bus', () => {
    expect(decodeWsNotification({
      jsonrpc: '2.0',
      method: 'agent/event',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'agent', sequence: 1 },
        event: {
          id: 'event-1',
          event: {
            type: 'TextChunk',
            session_id: 'session-1',
            turn_id: 'turn-1',
            round_id: 'round-1',
            text: 'hello',
          },
          priority: 'Normal',
          timestamp: { secs_since_epoch: 1, nanos_since_epoch: 0 },
        },
      },
    })).toEqual({
      event: 'agentic://text-chunk',
      payload: {
        sessionId: 'session-1',
        turnId: 'turn-1',
        roundId: 'round-1',
        text: 'hello',
      },
    });
  });

  it('projects the canonical SessionHistoryChanged transcript invalidation', () => {
    expect(decodeWsNotification({
      jsonrpc: '2.0',
      method: 'agent/event',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'agent', sequence: 2 },
        event: {
          id: 'event-history-1',
          event: {
            type: 'SessionHistoryChanged',
            session_id: 'session-1',
          },
          priority: 'Normal',
          timestamp: { secs_since_epoch: 1, nanos_since_epoch: 0 },
        },
      },
    })).toEqual({
      event: 'agentic://session-history-changed',
      payload: { sessionId: 'session-1' },
    });
  });

  it('keeps the canonical ToolEvent payload opaque while projecting envelope fields', () => {
    const toolEvent = {
      event_type: 'UserInputRequested',
      tool_id: 'question-1',
      tool_name: 'AskUserQuestion',
      registration_sequence: 7,
      params: {
        questions: [{ question: 'Continue?', header_text: 'Confirm' }],
      },
    };
    expect(decodeWsNotification({
      jsonrpc: '2.0',
      method: 'agent/event',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'agent', sequence: 3 },
        event: {
          id: 'event-tool-1',
          event: {
            type: 'ToolEvent',
            session_id: 'session-1',
            turn_id: 'turn-1',
            round_id: 'round-1',
            tool_event: toolEvent,
          },
          priority: 'Normal',
          timestamp: { secs_since_epoch: 1, nanos_since_epoch: 0 },
        },
      },
    })).toEqual({
      event: 'agentic://tool-event',
      payload: {
        sessionId: 'session-1',
        turnId: 'turn-1',
        roundId: 'round-1',
        toolEvent,
      },
    });
  });

  it('projects a real v4 agent/permissionEvent envelope to permission listeners', () => {
    const request = {
      requestId: 'permission-1',
      roundId: 'round-1',
      order: 0,
      projectId: 'project-1',
      sessionId: 'session-1',
      agentId: 'agentic',
      action: 'filesystem.read',
      resources: ['/workspace/file.txt'],
      source: { kind: 'tool_call', identity: 'tool-1' },
    };

    expect(decodeWsNotification({
      jsonrpc: '2.0',
      method: 'agent/permissionEvent',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'permission', sequence: 2 },
        event: { event: 'asked', request },
      },
    })).toEqual({
      event: 'permission://event',
      payload: { event: 'asked', request },
    });
  });

  it('projects typed config notifications to a stable frontend event', () => {
    expect(decodeWsNotification({
      jsonrpc: '2.0',
      method: 'config/event',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'config', sequence: 3 },
        event: { kind: 'modelConfigurationUpdated' },
      },
    })).toEqual({
      event: 'config://updated',
      payload: { kind: 'modelConfigurationUpdated' },
    });
  });

  it('projects v4 stream invalidation to an explicit transport event', () => {
    const params = {
      cursor: { connectionId: 'app-server-1', stream: 'agent', sequence: 4 },
      stream: 'agent',
      state: 'invalidated',
      resync: {
        method: 'session/sync',
        snapshotAvailable: false,
        reason: 'event buffer lagged',
      },
    };

    expect(decodeWsNotification({
      jsonrpc: '2.0',
      method: 'app/eventStreamState',
      params,
    })).toEqual({
      event: 'app-server://event-stream-state',
      payload: params,
    });
  });
});

describe('decodeResponseBody', () => {
  it('projects start_dialog_turn status to success/message', () => {
    expect(decodeResponseBody('start_dialog_turn', {
      status: 'started',
      sessionId: 's1',
      turnId: 't1',
    })).toEqual({ success: true, message: 'Dialog turn started' });
    expect(decodeResponseBody('start_dialog_turn', {
      status: 'queued',
      sessionId: 's1',
      turnId: 't1',
    })).toEqual({ success: true, message: 'Dialog turn queued' });
  });

  it('unwraps list_sessions into a bare array', () => {
    expect(decodeResponseBody('list_sessions', { sessions: [{ sessionId: 's1' }] }))
      .toEqual([{ sessionId: 's1' }]);
  });

  it('unwraps list_pending_permission_requests into a bare array', () => {
    expect(decodeResponseBody('list_pending_permission_requests', { requests: [{ requestId: 'r1' }] }))
      .toEqual([{ requestId: 'r1' }]);
  });

  it('unwraps respond_permission_batch into a bare string array', () => {
    expect(decodeResponseBody('respond_permission_batch', { request_ids: ['r1', 'r2'] }))
      .toEqual(['r1', 'r2']);
  });

  it('unwraps git_get_branches into a bare array', () => {
    expect(decodeResponseBody('git_get_branches', { branches: [{ name: 'main' }] }))
      .toEqual([{ name: 'main' }]);
  });

  it('passes unknown actions through unchanged', () => {
    const result = { foo: 'bar' };
    expect(decodeResponseBody('some_unknown_action', result)).toBe(result);
    expect(decodeResponseBody('get_config', result)).toBe(result);
  });
});

describe('WebSocketTransportAdapter protocol negotiation', () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('waits for a valid App Server v4 initialize response before sending business requests', async () => {
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();

    sockets[0].open();
    await Promise.resolve();

    expect(sockets[0].sent).toHaveLength(1);
    const initialize = sockets[0].sent[0];
    expect(initialize).toMatchObject({
      jsonrpc: '2.0',
      method: 'app/initialize',
      params: {
        protocolVersion: 4,
        client: {
          name: 'bitfun-web-ui',
        },
      },
    });

    let connected = false;
    void connection.then(() => {
      connected = true;
    });
    const businessRequest = adapter.request('list_sessions', {});
    await Promise.resolve();
    expect(connected).toBe(false);
    expect(adapter.isConnected()).toBe(false);
    expect(sockets[0].sent).toHaveLength(1);

    sockets[0].receive({
      jsonrpc: '2.0',
      id: initialize.id,
      result: validInitializeResult(),
    });
    await Promise.resolve();

    expect(connected).toBe(false);
    expect(adapter.isConnected()).toBe(false);
    expect(sockets[0].sent).toHaveLength(2);
    expect(sockets[0].sent[1]).toMatchObject({
      jsonrpc: '2.0',
      method: 'app/syncEvents',
      params: { streams: ['agent', 'permission', 'config'] },
    });

    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await connection;

    expect(connected).toBe(true);
    expect(adapter.isConnected()).toBe(true);
    await vi.waitFor(() => expect(sockets[0].sent).toHaveLength(3));
    expect(sockets[0].sent[2]).toMatchObject({
      jsonrpc: '2.0',
      method: 'agent/listSessions',
      params: {},
    });

    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[2].id,
      result: { sessions: [] },
    });
    await expect(businessRequest).resolves.toEqual([]);
  });

  it.each([
    {
      name: 'server maximum below client v4',
      result: { ...validInitializeResult(), protocolVersion: 3, minimumProtocolVersion: 3 },
      message: 'protocol version',
    },
    {
      name: 'server minimum above client v4',
      result: { ...validInitializeResult(), protocolVersion: 5, minimumProtocolVersion: 5 },
      message: 'protocol version',
    },
    {
      name: 'malformed capability descriptors',
      result: { ...validInitializeResult(), capabilities: null },
      message: 'capabilities',
    },
    {
      name: 'non-positive transport limit',
      result: {
        ...validInitializeResult(),
        limits: { maxFrameBytes: 0, eventBufferCapacity: 1024 },
      },
      message: 'transport limits',
    },
  ])('rejects $name before the connection becomes ready', async ({ result, message }) => {
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();

    sockets[0].open();
    const initialize = sockets[0].sent[0];
    sockets[0].receive({
      jsonrpc: '2.0',
      id: initialize.id,
      result,
    });

    await expect(connection).rejects.toThrow(message);
    expect(adapter.isConnected()).toBe(false);
    expect(sockets[0].sent).toHaveLength(1);
    await adapter.disconnect();
  });

  it('rejects an oversized UTF-8 business frame before the WebSocket is disrupted', async () => {
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(512),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await connection;

    await expect(adapter.request('list_sessions', {
      padding: '界'.repeat(512),
    })).rejects.toThrow('exceeds negotiated App Server frame limit');
    expect(sockets[0].sent).toHaveLength(2);
    expect(sockets[0].readyState).toBe(ProtocolMockWebSocket.OPEN);
    expect(adapter.isConnected()).toBe(true);
    await adapter.disconnect();
  });

  it('fails closed without reconnecting after an incompatible initialize result', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();

    sockets[0].open();
    const initialize = sockets[0].sent[0];
    sockets[0].receive({
      jsonrpc: '2.0',
      id: initialize.id,
      result: { ...validInitializeResult(), protocolVersion: 3, minimumProtocolVersion: 3 },
    });

    await expect(connection).rejects.toThrow('protocol version');
    expect(sockets[0].readyState).toBe(3);
    expect(vi.getTimerCount()).toBe(0);

    await vi.advanceTimersByTimeAsync(30_000);
    expect(sockets).toHaveLength(1);
  });

  it('fails closed when the server rejects initialize as non-retryable', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();

    sockets[0].open();
    const initialize = sockets[0].sent[0];
    sockets[0].receive({
      jsonrpc: '2.0',
      id: initialize.id,
      error: {
        code: -32602,
        message: 'Unsupported protocol version',
        data: { capability: 'app.initialize', retryable: false },
      },
    });

    await expect(connection).rejects.toThrow('Unsupported protocol version');
    expect(sockets[0].readyState).toBe(3);
    expect(vi.getTimerCount()).toBe(0);

    await vi.advanceTimersByTimeAsync(30_000);
    expect(sockets).toHaveLength(1);
  });

  it('renegotiates after reconnect and holds business requests until initialize succeeds', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const firstConnection = adapter.connect();

    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await firstConnection;

    sockets[0].close();
    await vi.advanceTimersByTimeAsync(1_000);
    expect(sockets).toHaveLength(2);

    sockets[1].open();
    const initialize = sockets[1].sent[0];
    expect(initialize).toMatchObject({
      method: 'app/initialize',
      params: { protocolVersion: 4 },
    });

    const businessRequest = adapter.request('list_sessions', {});
    await Promise.resolve();
    expect(adapter.isConnected()).toBe(false);
    expect(sockets[1].sent).toHaveLength(1);

    sockets[1].receive({
      jsonrpc: '2.0',
      id: initialize.id,
      result: validInitializeResult(),
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(adapter.isConnected()).toBe(false);
    expect(sockets[1].sent).toHaveLength(2);
    expect(sockets[1].sent[1]).toMatchObject({ method: 'app/syncEvents' });

    sockets[1].receive({
      jsonrpc: '2.0',
      id: sockets[1].sent[1].id,
      result: validSyncEventsResult({ agentSnapshotAvailable: true }),
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(adapter.isConnected()).toBe(true);
    expect(sockets[1].sent).toHaveLength(3);
    expect(sockets[1].sent[2]).toMatchObject({ method: 'agent/listSessions' });

    sockets[1].receive({
      jsonrpc: '2.0',
      id: sockets[1].sent[2].id,
      result: { sessions: [] },
    });
    await expect(businessRequest).resolves.toEqual([]);
    await adapter.disconnect();
  });

  it('reconciles permissions by requestId after reconnect and requires a Session owner refresh', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const permissionEvents: Array<Record<string, unknown>> = [];
    const refreshEvents: Array<Record<string, unknown>> = [];
    adapter.listen<Record<string, unknown>>('permission://event', event => {
      permissionEvents.push(event);
    });
    adapter.listen<Record<string, unknown>>(
      'app-server://session-owner-refresh-required',
      event => {
        refreshEvents.push(event);
      },
    );

    const firstConnection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await vi.advanceTimersByTimeAsync(0);
    const firstPermission = permissionRequest('permission-1');
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult({ pendingPermissions: [firstPermission] }),
    });
    await firstConnection;
    expect(permissionEvents).toEqual([
      { event: 'asked', request: firstPermission },
    ]);

    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'agent/permissionEvent',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'permission', sequence: 1 },
        event: { event: 'asked', request: firstPermission },
      },
    });
    expect(permissionEvents).toHaveLength(1);

    sockets[0].close();
    await vi.advanceTimersByTimeAsync(1_000);
    sockets[1].open();
    sockets[1].receive({
      jsonrpc: '2.0',
      id: sockets[1].sent[0].id,
      result: validInitializeResult(),
    });
    await vi.advanceTimersByTimeAsync(0);

    const waitingBusinessRequest = adapter.request('list_sessions', {});
    await vi.advanceTimersByTimeAsync(0);
    expect(sockets[1].sent).toHaveLength(2);

    const secondPermission = permissionRequest('permission-2');
    sockets[1].receive({
      jsonrpc: '2.0',
      id: sockets[1].sent[1].id,
      result: validSyncEventsResult({ pendingPermissions: [secondPermission] }),
    });

    await expect(waitingBusinessRequest).rejects.toThrow(
      'Reload the Web page',
    );
    expect(sockets[1].sent).toHaveLength(2);
    expect(permissionEvents).toEqual([
      { event: 'asked', request: firstPermission },
      {
        event: 'cancelled',
        requestId: 'permission-1',
        reason: 'Permission request is absent from the authoritative reconnect snapshot',
      },
      { event: 'asked', request: secondPermission },
    ]);
    expect(refreshEvents).toEqual([{
      generation: 2,
      stream: 'agent',
      reason: 'agent_snapshot_unavailable',
      resyncMethod: 'session/sync',
    }]);
    expect(sockets[1].readyState).toBe(3);
    expect(adapter.isConnected()).toBe(false);
    await vi.advanceTimersByTimeAsync(5_000);
    expect(sockets).toHaveLength(2);
    await adapter.disconnect();
  });

  it('fails closed on an agent stream invalidation without inventing a Session id', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const streamEvents: Array<Record<string, unknown>> = [];
    const refreshEvents: Array<Record<string, unknown>> = [];
    adapter.listen<Record<string, unknown>>('app-server://event-stream-state', event => {
      streamEvents.push(event);
    });
    adapter.listen<Record<string, unknown>>(
      'app-server://session-owner-refresh-required',
      event => {
        refreshEvents.push(event);
      },
    );

    const connection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await connection;

    const invalidation = {
      cursor: { connectionId: 'app-server-1', stream: 'agent', sequence: 9 },
      stream: 'agent',
      state: 'invalidated',
      resync: {
        method: 'session/sync',
        snapshotAvailable: false,
        reason: 'event buffer lagged',
      },
    };
    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'app/eventStreamState',
      params: invalidation,
    });

    expect(streamEvents).toEqual([invalidation]);
    expect(refreshEvents).toEqual([{
      generation: 1,
      stream: 'agent',
      reason: 'event_stream_invalidated',
      resyncMethod: 'session/sync',
    }]);
    expect(refreshEvents[0]).not.toHaveProperty('sessionId');
    await expect(adapter.request('list_sessions', {})).rejects.toThrow(
      'Reload the Web page',
    );
    expect(sockets[0].readyState).toBe(3);
    expect(adapter.isConnected()).toBe(false);
    await vi.advanceTimersByTimeAsync(5_000);
    expect(sockets).toHaveLength(1);
    await adapter.disconnect();
  });

  it('replays an existing reload-required state to a late App shell listener', async () => {
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await connection;

    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'app/eventStreamState',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'agent', sequence: 4 },
        stream: 'agent',
        state: 'invalidated',
        resync: {
          method: 'session/sync',
          snapshotAvailable: false,
          reason: 'event buffer lagged',
        },
      },
    });

    const refreshEvents: Array<Record<string, unknown>> = [];
    adapter.listen<Record<string, unknown>>(SESSION_OWNER_REFRESH_EVENT, event => {
      refreshEvents.push(event);
    });
    expect(refreshEvents).toEqual([{
      generation: 1,
      stream: 'agent',
      reason: 'event_stream_invalidated',
      resyncMethod: 'session/sync',
    }]);
    await adapter.disconnect();
  });

  it('reconciles the permission owner after a recoverable stream invalidation', async () => {
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const permissionEvents: Array<Record<string, unknown>> = [];
    adapter.listen<Record<string, unknown>>('permission://event', event => {
      permissionEvents.push(event);
    });

    const firstPermission = permissionRequest('permission-1');
    const connection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult({ pendingPermissions: [firstPermission] }),
    });
    await connection;

    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'app/eventStreamState',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'permission', sequence: 4 },
        stream: 'permission',
        state: 'lagged',
        resync: {
          method: 'app/syncEvents',
          snapshotAvailable: true,
          reason: 'event buffer lagged',
        },
      },
    });

    expect(sockets[0].sent).toHaveLength(3);
    expect(sockets[0].sent[2]).toMatchObject({
      method: 'app/syncEvents',
      params: { streams: ['permission'] },
    });
    const blockedBusinessRequest = adapter.request('list_sessions', {});
    await Promise.resolve();
    expect(sockets[0].sent).toHaveLength(3);
    const secondPermission = permissionRequest('permission-2');
    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'app/eventStreamState',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'permission', sequence: 5 },
        stream: 'permission',
        state: 'lagged',
        resync: {
          method: 'app/syncEvents',
          snapshotAvailable: true,
          reason: 'a second event buffer lag',
        },
      },
    });
    expect(sockets[0].sent).toHaveLength(3);
    const thirdPermission = permissionRequest('permission-3');
    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'agent/permissionEvent',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'permission', sequence: 6 },
        event: { event: 'asked', request: thirdPermission },
      },
    });
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[2].id,
      result: validSyncEventsResult({
        cursors: [
          { connectionId: 'app-server-1', stream: 'agent', sequence: 0 },
          { connectionId: 'app-server-1', stream: 'permission', sequence: 5 },
          { connectionId: 'app-server-1', stream: 'config', sequence: 0 },
        ],
        pendingPermissions: [secondPermission],
      }),
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(sockets[0].sent).toHaveLength(4);
    expect(sockets[0].sent[3]).toMatchObject({
      method: 'app/syncEvents',
      params: { streams: ['permission'] },
    });
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[3].id,
      result: validSyncEventsResult({
        cursors: [
          { connectionId: 'app-server-1', stream: 'agent', sequence: 0 },
          { connectionId: 'app-server-1', stream: 'permission', sequence: 6 },
          { connectionId: 'app-server-1', stream: 'config', sequence: 0 },
        ],
        pendingPermissions: [thirdPermission],
      }),
    });
    await vi.waitFor(() => expect(sockets[0].sent).toHaveLength(5));
    expect(sockets[0].sent[4]).toMatchObject({ method: 'agent/listSessions' });
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[4].id,
      result: { sessions: [] },
    });
    await expect(blockedBusinessRequest).resolves.toEqual([]);

    expect(permissionEvents).toEqual([
      { event: 'asked', request: firstPermission },
      {
        event: 'cancelled',
        requestId: 'permission-1',
        reason: 'Permission request is absent from the authoritative reconnect snapshot',
      },
      { event: 'asked', request: secondPermission },
      { event: 'asked', request: thirdPermission },
      {
        event: 'cancelled',
        requestId: 'permission-2',
        reason: 'Permission request is absent from the authoritative reconnect snapshot',
      },
    ]);
    expect(sockets[0].readyState).toBe(ProtocolMockWebSocket.OPEN);
    await adapter.disconnect();
  });

  it('reconnects instead of serving from a closed permission event source', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await connection;

    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'app/eventStreamState',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'permission', sequence: 4 },
        stream: 'permission',
        state: 'closed',
        resync: {
          method: 'app/syncEvents',
          snapshotAvailable: true,
          reason: 'permission owner closed',
        },
      },
    });

    expect(sockets[0].readyState).toBe(3);
    expect(adapter.isConnected()).toBe(false);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(sockets).toHaveLength(2);
    await adapter.disconnect();
  });

  it('starts a fresh permission snapshot when the resync socket closes', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await connection;

    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'app/eventStreamState',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'permission', sequence: 4 },
        stream: 'permission',
        state: 'lagged',
        resync: {
          method: 'app/syncEvents',
          snapshotAvailable: true,
          reason: 'event buffer lagged',
        },
      },
    });
    expect(sockets[0].sent).toHaveLength(3);
    sockets[0].receive({
      jsonrpc: '2.0',
      method: 'agent/permissionEvent',
      params: {
        cursor: { connectionId: 'app-server-1', stream: 'permission', sequence: 5 },
        event: { event: 'asked', request: permissionRequest('stale-permission') },
      },
    });

    sockets[0].close();
    await vi.advanceTimersByTimeAsync(1_000);
    expect(sockets).toHaveLength(2);
    sockets[1].open();
    sockets[1].receive({
      jsonrpc: '2.0',
      id: sockets[1].sent[0].id,
      result: validInitializeResult(),
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(sockets[1].sent).toHaveLength(2);
    expect(sockets[1].sent[1]).toMatchObject({ method: 'app/syncEvents' });
    sockets[1].receive({
      jsonrpc: '2.0',
      id: sockets[1].sent[1].id,
      result: validSyncEventsResult({
        agentSnapshotAvailable: true,
        cursors: [
          { connectionId: 'app-server-2', stream: 'agent', sequence: 0 },
          { connectionId: 'app-server-2', stream: 'permission', sequence: 0 },
          { connectionId: 'app-server-2', stream: 'config', sequence: 0 },
        ],
      }),
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(adapter.isConnected()).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
    await adapter.disconnect();
  });

  it('classifies a sent CreateSession with a lost response as outcome unknown', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await connection;

    let rejection: unknown;
    void adapter.request('create_session', {
      workspacePath: '/workspace',
      agentType: 'agentic',
    }).catch(error => {
      rejection = error;
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(sockets[0].sent[2]).toMatchObject({ method: 'agent/createSession' });
    sockets[0].close();
    await vi.advanceTimersByTimeAsync(0);

    try {
      expect(rejection).toBeInstanceOf(Error);
      expect((rejection as Error).message).toContain('outcome_unknown:');
    } finally {
      await adapter.disconnect();
    }
  });

  it('gates subsequent business after a sent side effect times out', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const refreshEvents: Array<Record<string, unknown>> = [];
    adapter.listen<Record<string, unknown>>(SESSION_OWNER_REFRESH_EVENT, event => {
      refreshEvents.push(event);
    });
    const connection = adapter.connect();
    sockets[0].open();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[0].id,
      result: validInitializeResult(),
    });
    await Promise.resolve();
    sockets[0].receive({
      jsonrpc: '2.0',
      id: sockets[0].sent[1].id,
      result: validSyncEventsResult(),
    });
    await connection;

    const create = adapter.request('create_session', {
      workspacePath: '/workspace',
      agentType: 'agentic',
    });
    const createFailure = expect(create).rejects.toThrow('outcome_unknown:');
    await vi.advanceTimersByTimeAsync(30_000);
    await createFailure;

    expect(refreshEvents).toEqual([{
      generation: 1,
      stream: 'app',
      reason: 'side_effect_outcome_unknown',
      resyncMethod: 'reload',
    }]);
    await expect(adapter.request('list_sessions', {})).rejects.toThrow(
      'Reload the Web page',
    );
    expect(sockets[0].sent).toHaveLength(3);
    await adapter.disconnect();
  });

  it('cleans up a timed-out initialize before retrying on a fresh connection', async () => {
    vi.useFakeTimers();
    const sockets = installProtocolWebSocket();
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();

    sockets[0].open();
    const initialize = sockets[0].sent[0];
    const businessRequest = adapter.request('list_sessions', {});
    const connectionFailure = expect(connection).rejects.toThrow(
      'Request timeout: app/initialize'
    );
    const businessFailure = expect(businessRequest).rejects.toThrow(
      'Request timeout: app/initialize'
    );

    await vi.advanceTimersByTimeAsync(30_000);
    await connectionFailure;
    await businessFailure;
    expect(sockets[0].readyState).toBe(3);
    expect(vi.getTimerCount()).toBe(1);

    // A late response from the failed connection must not revive it or replay
    // the business request that was waiting on that connection.
    sockets[0].receive({
      jsonrpc: '2.0',
      id: initialize.id,
      result: validInitializeResult(),
    });
    expect(adapter.isConnected()).toBe(false);

    await vi.advanceTimersByTimeAsync(1_000);
    expect(sockets).toHaveLength(2);
    sockets[1].open();
    expect(sockets[1].sent).toHaveLength(1);
    expect(sockets[1].sent[0]).toMatchObject({ method: 'app/initialize' });

    sockets[1].receive({
      jsonrpc: '2.0',
      id: sockets[1].sent[0].id,
      result: validInitializeResult(),
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(adapter.isConnected()).toBe(false);
    expect(sockets[1].sent).toHaveLength(2);
    expect(sockets[1].sent[1]).toMatchObject({ method: 'app/syncEvents' });
    sockets[1].receive({
      jsonrpc: '2.0',
      id: sockets[1].sent[1].id,
      result: validSyncEventsResult(),
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(adapter.isConnected()).toBe(true);
    expect(sockets[1].sent).toHaveLength(2);
    await adapter.disconnect();
  });
});

describe('WebSocketTransportAdapter reconnect lifecycle', () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('cancels a scheduled reconnect when explicitly disconnected', async () => {
    vi.useFakeTimers();

    const sockets: MockWebSocket[] = [];
    class MockWebSocket {
      static readonly OPEN = 1;

      readyState = 0;
      sent: Array<Record<string, unknown>> = [];
      onopen: ((event: Event) => void) | null = null;
      onerror: ((event: Event) => void) | null = null;
      onclose: ((event: CloseEvent) => void) | null = null;
      onmessage: ((event: MessageEvent) => void) | null = null;

      constructor(readonly url: string) {
        sockets.push(this);
      }

      open(): void {
        this.readyState = MockWebSocket.OPEN;
        this.onopen?.({} as Event);
      }

      async completeInitialize(): Promise<void> {
        const initialize = this.sent[0];
        this.onmessage?.({
          data: JSON.stringify({
            jsonrpc: '2.0',
            id: initialize.id,
            result: {
              protocolVersion: 4,
              minimumProtocolVersion: 4,
              server: { name: 'bitfun-app-server', version: '0.2.18' },
              capabilities: [],
              limits: { maxFrameBytes: 16 * 1024 * 1024, eventBufferCapacity: 1024 },
            },
          }),
        } as MessageEvent);
        await Promise.resolve();
        const syncEvents = this.sent[1];
        this.onmessage?.({
          data: JSON.stringify({
            jsonrpc: '2.0',
            id: syncEvents.id,
            result: validSyncEventsResult(),
          }),
        } as MessageEvent);
      }

      closeFromServer(): void {
        this.readyState = 3;
        this.onclose?.({} as CloseEvent);
      }

      close(): void {
        this.readyState = 3;
        this.onclose?.({} as CloseEvent);
      }

      send(payload: string): void {
        this.sent.push(JSON.parse(payload) as Record<string, unknown>);
      }
    }

    vi.stubGlobal('WebSocket', MockWebSocket);
    const adapter = new WebSocketTransportAdapter('ws://example.test/ws');
    const connection = adapter.connect();

    sockets[0].open();
    await sockets[0].completeInitialize();
    await connection;
    sockets[0].closeFromServer();
    expect(vi.getTimerCount()).toBe(1);

    await adapter.disconnect();
    expect(vi.getTimerCount()).toBe(0);

    await vi.advanceTimersByTimeAsync(1_000);
    expect(sockets).toHaveLength(1);
  });
});
