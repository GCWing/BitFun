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

  it('never reuses an existing session', () => {
    const workspace = createWorkspace();
    const codeSession = createSession({
      sessionId: 'code-session',
      lastActiveAt: 5,
    });

    flowChatStore.setState(() => ({
      sessions: new Map([[codeSession.sessionId, codeSession]]),
      activeSessionId: codeSession.sessionId,
    }));

    expect(findReusableEmptySessionId(workspace, 'agentic')).toBeNull();
  });
});
