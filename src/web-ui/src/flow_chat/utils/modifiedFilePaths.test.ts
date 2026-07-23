import { describe, expect, it } from 'vitest';
import type { DialogTurn } from '../types/flow-chat';
import {
  collectModifiedFilePathsFromTurns,
  hasOpaqueWorkspaceMutationRisk,
} from './modifiedFilePaths';

function turn(id: string, items: unknown[]): DialogTurn {
  return {
    id,
    sessionId: 'review-session',
    userMessage: { id: `user-${id}`, content: id, timestamp: 1 },
    modelRounds: [{
      id: `round-${id}`,
      index: 0,
      status: 'completed',
      startTime: 1,
      isStreaming: false,
      isComplete: true,
      items,
    }],
    status: 'completed',
    startTime: 1,
  } as DialogTurn;
}

function tool(toolName: string, input: Record<string, unknown>, success = true) {
  return {
    id: `${toolName}-${String(input.file_path ?? input.filePath ?? input.path ?? 'tool')}`,
    type: 'tool',
    timestamp: 1,
    status: success ? 'completed' : 'error',
    toolName,
    toolCall: { id: 'tool-call', input },
    toolResult: { success, result: null },
  };
}

describe('collectModifiedFilePathsFromTurns', () => {
  it('collects successful file mutations after the remediation baseline', () => {
    const paths = collectModifiedFilePathsFromTurns([
      turn('baseline', [tool('Write', { file_path: 'src/before.ts' })]),
      turn('fix', [
        tool('Write', { file_path: 'D:/workspace/project/src/auth.ts' }),
        tool('Write', { payload: '+++ D:/workspace/project/src/combined.ts\nexport {};' }),
        tool('Write', { payload: '+++ D:/workspace/project/src/empty.ts' }),
        tool('Edit', { filePath: 'src/helper.ts' }),
        tool('write_file', { path: 'src/auth.ts' }),
        tool('Write', { file_path: 'src/failed.ts' }, false),
        tool('Exec', { command: 'pnpm test' }),
      ]),
    ], 'baseline', 'D:/workspace/project');

    expect(paths).toEqual(['src/auth.ts', 'src/combined.ts', 'src/empty.ts', 'src/helper.ts']);
  });

  it('marks command and Git tools as requiring a conservative workspace fallback', () => {
    const turns = [turn('fix', [
      tool('ExecCommand', { command: 'pnpm format' }),
      tool('Git', { args: ['apply', 'fix.patch'] }),
    ])];

    expect(hasOpaqueWorkspaceMutationRisk(turns, null)).toBe(true);
  });

  it('keeps the conservative fallback when an opaque tool fails or is interrupted', () => {
    const failedCommand = tool('ExecCommand', { command: 'pnpm format' }, false);
    const runningCommand = {
      ...tool('WriteStdin', { content: 'continue' }),
      status: 'running',
      toolResult: undefined,
    };

    expect(hasOpaqueWorkspaceMutationRisk([
      turn('failed-fix', [failedCommand]),
    ], null)).toBe(true);
    expect(hasOpaqueWorkspaceMutationRisk([
      turn('interrupted-fix', [runningCommand]),
    ], null)).toBe(true);
  });
});
