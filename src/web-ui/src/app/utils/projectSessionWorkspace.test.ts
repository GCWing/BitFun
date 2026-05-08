import { afterEach, describe, expect, it } from 'vitest';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { FlowChatState, Session } from '@/flow_chat/types/flow-chat';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';
import { findReusableEmptySessionId } from './projectSessionWorkspace';

const resetStore = () => {
  flowChatStore.setState((): FlowChatState => ({
    sessions: new Map(),
    activeSessionId: null,
  }));
};

const createWorkspace = (): WorkspaceInfo => ({
  id: 'workspace-1',
  name: 'BitFun',
  rootPath: '/workspace/BitFun',
  workspaceKind: WorkspaceKind.Normal,
});

const createSession = (overrides: Partial<Session> = {}): Session => ({
  sessionId: 'session-1',
  title: 'Session 1',
  dialogTurns: [],
  status: 'idle',
  config: { agentType: 'agentic' },
  createdAt: 1,
  lastActiveAt: 1,
  error: null,
  isHistorical: false,
  maxContextTokens: 128128,
  mode: 'agentic',
  workspacePath: '/workspace/BitFun',
  workspaceId: 'workspace-1',
  sessionKind: 'normal',
  btwThreads: [],
  isTransient: false,
  ...overrides,
});

describe('findReusableEmptySessionId', () => {
  afterEach(() => {
    resetStore();
  });

  it('does not reuse an empty ACP session for a new code session', () => {
    const workspace = createWorkspace();
    const acpSession = createSession({
      sessionId: 'acp-session',
      config: { agentType: 'acp:codex' },
      mode: 'acp:codex',
      lastActiveAt: 10,
    });

    flowChatStore.setState(() => ({
      sessions: new Map([[acpSession.sessionId, acpSession]]),
      activeSessionId: acpSession.sessionId,
    }));

    expect(findReusableEmptySessionId(workspace, 'agentic')).toBeNull();
  });

  it('still reuses a matching empty code session when ACP sessions also exist', () => {
    const workspace = createWorkspace();
    const codeSession = createSession({
      sessionId: 'code-session',
      lastActiveAt: 5,
    });
    const acpSession = createSession({
      sessionId: 'acp-session',
      config: { agentType: 'acp:codex' },
      mode: 'acp:codex',
      lastActiveAt: 20,
    });

    flowChatStore.setState(() => ({
      sessions: new Map([
        [codeSession.sessionId, codeSession],
        [acpSession.sessionId, acpSession],
      ]),
      activeSessionId: acpSession.sessionId,
    }));

    expect(findReusableEmptySessionId(workspace, 'agentic')).toBe(codeSession.sessionId);
  });
});
