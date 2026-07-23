import { describe, expect, it } from 'vitest';
import {
  buildMiniAppCustomizationSessionRequest,
  createMiniAppCustomizationSessionId,
} from './miniAppCustomizationSession';

describe('buildMiniAppCustomizationSessionRequest', () => {
  it('creates a hidden subagent session request for MiniApp customization', () => {
    expect(buildMiniAppCustomizationSessionRequest({
      sessionId: 'miniapp-customize-builtin-gomoku-1',
      sessionName: 'Customize Gomoku',
      workspacePath: 'D:/workspace/BitFun',
    })).toMatchObject({
      sessionId: 'miniapp-customize-builtin-gomoku-1',
      sessionName: 'Customize Gomoku',
      agentType: 'agentic',
      workspacePath: 'D:/workspace/BitFun',
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
