import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import {
  hasInterruptedTurnHoldingQueue,
  selectInterruptedTurnRecovery,
} from './interruptedTurnRecovery';

function interruptedSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    dialogTurns: [{
      id: 'turn-1',
      sessionId: 'session-1',
      agentType: 'agentic',
      userMessage: { id: 'user-1', content: 'finish the task', timestamp: 1 },
      modelRounds: [],
      status: 'cancelled',
      finishReason: 'interrupted',
      recovery: {
        status: 'interrupted',
        executionGeneration: 2,
        resumeCount: 2,
        modelId: 'model-a',
      },
      startTime: 1,
    }],
    status: 'idle',
    config: { agentType: 'agentic', modelName: 'model-a', workspacePath: 'D:/workspace' },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    mode: 'agentic',
    sessionKind: 'normal',
    ...overrides,
  };
}

const emptyComposer = {
  draft: '',
  hasComposerAttachments: false,
  executionIdle: true,
  desktopRuntime: true,
  peerMode: false,
  acpSession: false,
  modeChangePending: false,
  modelChangePending: false,
};

describe('selectInterruptedTurnRecovery', () => {
  it('returns the latest interrupted turn and generation for an empty local composer', () => {
    expect(selectInterruptedTurnRecovery(interruptedSession(), emptyComposer)).toEqual({
      sessionId: 'session-1',
      turnId: 'turn-1',
      executionGeneration: 2,
    });
  });

  it.each([
    ['draft text', { draft: 'new request' }],
    ['attachments', { hasComposerAttachments: true }],
    ['remote workspace', {}, { remoteConnectionId: 'remote-1' }],
    ['peer mode', { peerMode: true }],
    ['web runtime', { desktopRuntime: false }],
    ['ACP session', { acpSession: true }],
    ['active goal', {}, { threadGoal: { goalId: 'goal-1', objective: 'ship', status: 'active' } }],
    ['mode change', {}, { mode: 'review' }],
    ['model change', {}, { config: { agentType: 'agentic', modelName: 'model-b' } }],
    ['pending mode update', { modeChangePending: true }],
  ])('hides recovery for %s', (_label, optionOverrides, sessionOverrides = {}) => {
    expect(selectInterruptedTurnRecovery(
      interruptedSession(sessionOverrides as Partial<Session>),
      { ...emptyComposer, ...optionOverrides },
    )).toBeNull();
  });

  it('rejects stale, non-latest, recovering, and ordinary cancelled turns', () => {
    const base = interruptedSession();
    expect(selectInterruptedTurnRecovery({
      ...base,
      dialogTurns: [
        base.dialogTurns[0],
        { ...base.dialogTurns[0], id: 'turn-2', finishReason: 'cancelled', recovery: undefined },
      ],
    }, emptyComposer)).toBeNull();
    expect(selectInterruptedTurnRecovery({
      ...base,
      dialogTurns: [{
        ...base.dialogTurns[0],
        status: 'processing',
        recovery: { ...base.dialogTurns[0].recovery!, status: 'recovering' },
      }],
    }, emptyComposer)).toBeNull();
  });

  it('holds pending input only while the latest turn is awaiting a recovery decision', () => {
    const session = interruptedSession();
    expect(hasInterruptedTurnHoldingQueue(session)).toBe(true);
    expect(hasInterruptedTurnHoldingQueue({
      ...session,
      dialogTurns: [{
        ...session.dialogTurns[0],
        recovery: { ...session.dialogTurns[0].recovery!, status: 'recovering' },
      }],
    })).toBe(false);
  });
});
