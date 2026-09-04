import { afterEach, describe, expect, it } from 'vitest';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { FlowChatState, Session } from '@/flow_chat/types/flow-chat';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';
import {
  findReusableEmptySessionId,
  pickPrimaryAssistantWorkspace,
} from './projectSessionWorkspace';

const resetStore = () => {
  flowChatStore.setState((): FlowChatState => ({
    sessions: new Map(),
    activeSessionId: null,
  }));
};

const createWorkspace = (): WorkspaceInfo => ({
  id: 'workspace-1',
  name: 'OpenBitFun',
  rootPath: '/workspace/OpenBitFun',
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
  workspacePath: '/workspace/OpenBitFun',
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

describe('pickPrimaryAssistantWorkspace', () => {
  const createAssistantWorkspace = (
    id: string,
    assistantId?: string,
  ): WorkspaceInfo => ({
    id,
    name: id,
    rootPath: `/assistants/${id}`,
    workspaceKind: WorkspaceKind.Assistant,
    assistantId,
  });

  it('selects the primary assistant even when a named assistant appears first', () => {
    const namedAssistant = createAssistantWorkspace('named', 'assistant-1');
    const primaryAssistant = createAssistantWorkspace('primary');

    expect(
      pickPrimaryAssistantWorkspace([namedAssistant, primaryAssistant])
    ).toBe(primaryAssistant);
  });

  it('selects the configured primary assistant by workspace id', () => {
    const firstAssistant = createAssistantWorkspace('first', 'assistant-1');
    const configuredPrimary = createAssistantWorkspace('configured', 'assistant-2');

    expect(pickPrimaryAssistantWorkspace([firstAssistant, configuredPrimary], 'configured'))
      .toBe(configuredPrimary);
  });

  it('does not fall back to a named assistant', () => {
    const namedAssistant = createAssistantWorkspace('named', 'assistant-1');

    expect(pickPrimaryAssistantWorkspace([namedAssistant])).toBeNull();
  });
});
