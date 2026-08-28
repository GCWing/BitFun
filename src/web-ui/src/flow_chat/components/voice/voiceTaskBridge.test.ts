import { describe, expect, it } from 'vitest';
import type { Session } from '@/flow_chat/types/flow-chat';
import {
  extractVoiceTaskProgressTexts,
  extractVoiceTaskSummary,
  summarizeVoiceTaskProgress,
} from './voiceTaskBridge';

function sessionWithItems(
  items: Array<Record<string, unknown>>,
  turnStatus: 'processing' | 'finishing' | 'completed' = 'completed',
): Session {
  return {
    sessionId: 'voice-task-session',
    dialogTurns: [{
      id: 'turn-1',
      sessionId: 'voice-task-session',
      userMessage: { id: 'user-1', content: 'do the work', timestamp: 1 },
      modelRounds: [{
        id: 'round-1',
        index: 0,
        items,
        isStreaming: false,
        isComplete: true,
        status: 'completed',
        startTime: 1,
      }],
      status: turnStatus,
      startTime: 1,
    }],
  } as unknown as Session;
}

describe('extractVoiceTaskSummary', () => {
  it('returns assistant text without exposing thinking or tool payloads', () => {
    const session = sessionWithItems([
      { id: 'thinking', type: 'thinking', content: 'private reasoning', status: 'completed' },
      {
        id: 'tool',
        type: 'tool',
        toolName: 'Shell',
        status: 'completed',
        toolCall: { id: 'call-1', input: { command: 'secret command' } },
      },
      {
        id: 'text',
        type: 'text',
        content: '## Done\n\nUpdated **two files** and ran `tests`.',
        status: 'completed',
        isStreaming: false,
      },
    ]);

    const summary = extractVoiceTaskSummary(session);
    expect(summary).toBe('Done Updated two files and ran tests.');
    expect(summary).not.toContain('private reasoning');
    expect(summary).not.toContain('secret command');
  });

  it('provides a stable fallback when the task has no final text', () => {
    expect(extractVoiceTaskSummary(sessionWithItems([])))
      .toBe('BitFun completed the task without a text response.');
  });

  it('extracts only completed public text while a task is running', () => {
    const updates = extractVoiceTaskProgressTexts(sessionWithItems([
      {
        id: 'thinking',
        type: 'thinking',
        content: 'private reasoning',
        status: 'completed',
      },
      {
        id: 'streaming',
        type: 'text',
        content: 'partial sentence',
        status: 'streaming',
        isStreaming: true,
      },
      {
        id: 'progress',
        type: 'text',
        content: '## Progress\n\nFinished **reading the config** and am checking `audio` output.',
        status: 'completed',
        isStreaming: false,
      },
    ], 'processing'));

    expect(updates).toEqual([{
      id: 'round-1:progress:Progress: Finished reading the config. Now checking audio output.',
      text: 'Progress: Finished reading the config. Now checking audio output.',
    }]);
  });

  it('does not announce final text as an in-flight progress update', () => {
    expect(extractVoiceTaskProgressTexts(sessionWithItems([{
      id: 'final',
      type: 'text',
      content: 'Done.',
      status: 'completed',
      isStreaming: false,
    }], 'completed'))).toEqual([]);
  });

  it('keeps announcing completed public progress while the turn is finishing', () => {
    expect(extractVoiceTaskProgressTexts(sessionWithItems([{
      id: 'verification',
      type: 'text',
      content: 'Tests are complete. Preparing the final result.',
      status: 'completed',
      isStreaming: false,
    }], 'finishing'))).toEqual([{
      id: 'round-1:verification:Progress: Tests are complete. Preparing the final result.',
      text: 'Progress: Tests are complete. Preparing the final result.',
    }]);
  });

  it('rewrites long Agent prose into a one-to-two sentence spoken brief', () => {
    const original = '我已经完成了配置文件读取和依赖检查，确认主流程没有问题。接下来我会继续检查音频输出链路，并运行相关测试验证结果。这里还有一长段不需要播报的实现细节。';

    const spoken = summarizeVoiceTaskProgress(original);

    expect(spoken).toBe('进展：配置文件读取和依赖检查已完成，确认主流程没有问题。下一步继续检查音频输出链路，并运行相关测试验证结果。');
    expect(spoken).not.toBe(original);
    expect(spoken.match(/[。！？!?]/g)?.length).toBeLessThanOrEqual(2);
    expect(spoken.length).toBeLessThanOrEqual(90);
  });
});
