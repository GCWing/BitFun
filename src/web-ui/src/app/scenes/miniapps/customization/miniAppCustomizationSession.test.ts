import { describe, expect, it } from 'vitest';
import {
  buildMiniAppCustomizationSessionRequest,
  createMiniAppCustomizationSessionId,
  isMiniAppCustomizationSessionRunning,
} from './miniAppCustomizationSession';
import type { DialogTurn, Session } from '@/flow_chat/types/flow-chat';

describe('buildMiniAppCustomizationSessionRequest', () => {
  it('creates a hidden subagent session request for MiniApp customization', () => {
    expect(buildMiniAppCustomizationSessionRequest({
      sessionId: 'miniapp-customize-builtin-gomoku-1',
      sessionName: 'Customize Gomoku',
      workspacePath: 'D:/workspace/OpenBitFun',
    })).toMatchObject({
      sessionId: 'miniapp-customize-builtin-gomoku-1',
      sessionName: 'Customize Gomoku',
      agentType: 'agentic',
      workspacePath: 'D:/workspace/OpenBitFun',
      sessionKind: 'subagent',
      config: {
        enableTools: true,
        safeMode: true,
        autoCompact: true,
        enableContextCompression: true,
      },
    });
  });

  it('generates a portable session identifier', () => {
    expect(createMiniAppCustomizationSessionId('builtin-gomoku')).toMatch(
      /^miniapp-customize-builtin-gomoku-\d+$/,
    );
  });
});

describe('isMiniAppCustomizationSessionRunning', () => {
  function sessionWithTurnStatus(status: DialogTurn['status']): Pick<Session, 'dialogTurns'> {
    return {
      dialogTurns: [{ status } as DialogTurn],
    };
  }

  it.each([
    'pending',
    'image_analyzing',
    'processing',
    'finishing',
    'cancelling',
  ] satisfies DialogTurn['status'][])('blocks draft actions while the Agent turn is %s', (status) => {
    expect(isMiniAppCustomizationSessionRunning(sessionWithTurnStatus(status))).toBe(true);
  });

  it.each([
    'completed',
    'cancelled',
    'error',
  ] satisfies DialogTurn['status'][])('unblocks draft actions after the Agent turn is %s', (status) => {
    expect(isMiniAppCustomizationSessionRunning(sessionWithTurnStatus(status))).toBe(false);
  });

  it('treats an absent or empty session as idle', () => {
    expect(isMiniAppCustomizationSessionRunning(null)).toBe(false);
    expect(isMiniAppCustomizationSessionRunning({ dialogTurns: [] })).toBe(false);
  });
});
