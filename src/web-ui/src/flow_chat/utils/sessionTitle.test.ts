import { describe, expect, it } from 'vitest';
import type { SessionMetadata } from '@/shared/types/session-history';
import {
  deriveSessionTitleStateFromMetadata,
  freezeSessionTitleState,
  getNextDefaultSessionTitleCount,
  resolvePersistedSessionTitle,
  resolveSessionTitle,
} from './sessionTitle';
import type { Session } from '../types/flow-chat';

function createTranslator(locale: 'en' | 'zh') {
  return (key: string, options?: Record<string, unknown>) => {
    const count = options?.count;
    if (key === 'flow-chat:session.newCodeWithIndex') {
      return locale === 'zh' ? `新建代码会话 ${count}` : `New Code Session ${count}`;
    }
    if (key === 'flow-chat:session.new') {
      return locale === 'zh' ? '新建会话' : 'New Session';
    }
    return key;
  };
}

function counterSession(overrides: Partial<Session>): Session {
  return {
    sessionId: overrides.sessionId ?? 'session',
    title: overrides.title ?? 'New Code Session',
    titleSource: overrides.titleSource ?? 'i18n',
    titleI18nKey: overrides.titleI18nKey ?? 'flow-chat:session.newCodeWithIndex',
    titleI18nParams: overrides.titleI18nParams ?? { count: 1 },
    dialogTurns: [],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    mode: 'agentic',
    sessionKind: 'normal',
    ...overrides,
  } as Session;
}

describe('sessionTitle', () => {
  it('renders untouched default session titles from the current locale', () => {
    const session = {
      title: 'New Code Session 2',
      titleSource: 'i18n',
      titleI18nKey: 'flow-chat:session.newCodeWithIndex',
      titleI18nParams: { count: 2 },
    } as const;

    expect(resolveSessionTitle(session, createTranslator('en'))).toBe('New Code Session 2');
    expect(resolveSessionTitle(session, createTranslator('zh'))).toBe('新建代码会话 2');
  });

  it('keeps generated titles fixed after the first message', () => {
    const frozen = freezeSessionTitleState('Fix flaky test');

    expect(resolveSessionTitle(frozen, createTranslator('en'))).toBe('Fix flaky test');
    expect(resolveSessionTitle(frozen, createTranslator('zh'))).toBe('Fix flaky test');
  });

  it('restores persisted locale-aware titles only for untouched sessions', () => {
    const untouchedMetadata: SessionMetadata = {
      sessionId: 'session-1',
      sessionName: 'New Code Session 3',
      agentType: 'agentic',
      modelName: 'auto',
      createdAt: 1,
      lastActiveAt: 2,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: {
        kind: 'normal',
        titleSource: 'i18n',
        titleKey: 'flow-chat:session.newCodeWithIndex',
        titleParams: { count: 3 },
      },
    };
    const activeMetadata: SessionMetadata = {
      ...untouchedMetadata,
      turnCount: 1,
      sessionName: 'Fix flaky test',
    };

    expect(deriveSessionTitleStateFromMetadata(untouchedMetadata)).toMatchObject({
      titleSource: 'i18n',
      titleI18nKey: 'flow-chat:session.newCodeWithIndex',
      titleI18nParams: { count: 3 },
    });
    expect(resolvePersistedSessionTitle(untouchedMetadata, createTranslator('zh'))).toBe(
      '新建代码会话 3',
    );

    expect(deriveSessionTitleStateFromMetadata(activeMetadata)).toMatchObject({
      title: 'Fix flaky test',
      titleSource: 'text',
      titleI18nKey: undefined,
    });
    expect(resolvePersistedSessionTitle(activeMetadata, createTranslator('zh'))).toBe(
      'Fix flaky test',
    );
  });

  it('keeps default title counters separate for each workspace and mode', () => {
    const sessions = [
      counterSession({
        sessionId: 'a-code-1',
        workspaceId: 'workspace-a',
        workspacePath: 'D:/workspace/a',
        mode: 'agentic',
        titleI18nParams: { count: 1 },
      }),
      counterSession({
        sessionId: 'b-code-1',
        workspaceId: 'workspace-b',
        workspacePath: 'D:/workspace/b',
        mode: 'agentic',
        titleI18nParams: { count: 1 },
      }),
      counterSession({
        sessionId: 'a-cowork-4',
        workspaceId: 'workspace-a',
        workspacePath: 'D:/workspace/a',
        mode: 'Cowork',
        titleI18nKey: 'flow-chat:session.newCoworkWithIndex',
        titleI18nParams: { count: 4 },
      }),
    ];

    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'code',
        workspaceId: 'workspace-a',
        workspacePath: 'D:/workspace/a',
      }),
    ).toBe(2);
    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'code',
        workspaceId: 'workspace-b',
        workspacePath: 'D:/workspace/b',
      }),
    ).toBe(2);
    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'cowork',
        workspaceId: 'workspace-a',
        workspacePath: 'D:/workspace/a',
      }),
    ).toBe(5);
  });

  it('continues from the highest title count in the same workspace scope', () => {
    const sessions = [
      counterSession({
        sessionId: 'code-2',
        workspacePath: 'D:/workspace/a',
        titleI18nParams: { count: 2 },
      }),
      counterSession({
        sessionId: 'code-5',
        workspacePath: 'D:/workspace/a',
        titleI18nParams: { count: 5 },
      }),
    ];

    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'code',
        workspacePath: 'D:/workspace/a',
      }),
    ).toBe(6);
  });

  it('counts generated same-scope sessions so numbering does not reset after title generation', () => {
    const sessions = [
      counterSession({
        sessionId: 'generated-code',
        workspacePath: 'D:/workspace/a',
        title: 'Fix flaky test',
        titleSource: 'text',
        titleI18nKey: undefined,
        titleI18nParams: undefined,
      }),
    ];

    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'code',
        workspacePath: 'D:/workspace/a',
      }),
    ).toBe(2);
  });

  it('keeps remote workspace counters separate by host and path', () => {
    const sessions = [
      counterSession({
        sessionId: 'host-a-code-2',
        workspacePath: '/repo',
        remoteConnectionId: 'ssh-user@host-a:22',
        remoteSshHost: 'host-a',
        titleI18nParams: { count: 2 },
      }),
      counterSession({
        sessionId: 'host-b-code-1',
        workspacePath: '/repo',
        remoteConnectionId: 'ssh-user@host-b:22',
        remoteSshHost: 'host-b',
        titleI18nParams: { count: 1 },
      }),
    ];

    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'code',
        workspacePath: '/repo',
        remoteConnectionId: 'ssh-user@host-a:22',
        remoteSshHost: 'host-a',
      }),
    ).toBe(3);
    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'code',
        workspacePath: '/repo',
        remoteConnectionId: 'ssh-user@host-b:22',
        remoteSshHost: 'host-b',
      }),
    ).toBe(2);
  });

  it('keeps remote workspace counters stable across legacy and portless connection ids', () => {
    const sessions = [
      counterSession({
        sessionId: 'legacy-remote-code-2',
        workspacePath: '/repo',
        remoteConnectionId: 'ssh-user@host-a:22',
        remoteSshHost: undefined,
        titleI18nParams: { count: 2 },
      }),
    ];

    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'code',
        workspacePath: '/repo',
        remoteConnectionId: 'ssh-user@host-a',
      }),
    ).toBe(3);
  });

  it('ignores child sessions when choosing a main session title count', () => {
    const sessions = [
      counterSession({
        sessionId: 'normal-code-1',
        workspacePath: 'D:/workspace/a',
        titleI18nParams: { count: 1 },
      }),
      counterSession({
        sessionId: 'review-code-9',
        workspacePath: 'D:/workspace/a',
        sessionKind: 'review',
        titleI18nParams: { count: 9 },
      }),
    ];

    expect(
      getNextDefaultSessionTitleCount(sessions, {
        mode: 'code',
        workspacePath: 'D:/workspace/a',
      }),
    ).toBe(2);
  });
});
