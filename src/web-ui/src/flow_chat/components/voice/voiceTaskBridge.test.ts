import { describe, expect, it } from 'vitest';
import type { Session } from '@/flow_chat/types/flow-chat';
import {
  extractVoiceTaskConclusion,
  extractVoiceTaskProgressTexts,
  extractVoiceTaskSummary,
  summarizeVoiceTaskConclusion,
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
      id: 'round-1:progress:Finished reading the config. Now checking audio output.',
      text: 'Finished reading the config. Now checking audio output.',
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
      id: 'round-1:verification:Tests are complete. Preparing the final result.',
      text: 'Tests are complete. Preparing the final result.',
    }]);
  });

  it('rewrites long Agent prose into a one-to-two sentence spoken brief', () => {
    const original = '我已经完成了配置文件读取和依赖检查，确认主流程没有问题。接下来我会继续检查音频输出链路，并运行相关测试验证结果。这里还有一长段不需要播报的实现细节。';

    const spoken = summarizeVoiceTaskProgress(original);

    expect(spoken).toBe('配置文件读取和依赖检查已完成，确认主流程没有问题。下一步继续检查音频输出链路，并运行相关测试验证结果。');
    expect(spoken).not.toMatch(/^(?:进展|Progress)[:：]/i);
    expect(spoken).not.toBe(original);
    expect(spoken.match(/[。！？!?]/g)?.length).toBeLessThanOrEqual(2);
    expect(spoken.length).toBeLessThanOrEqual(90);
  });

  it('removes a progress label already present in Agent text', () => {
    expect(summarizeVoiceTaskProgress('进展：已经完成了配置检查。接下来会运行测试。'))
      .toBe('配置检查已完成。下一步运行测试。');
    expect(summarizeVoiceTaskProgress('Progress: Finished the config check. Next, I will run tests.'))
      .toBe('Finished the config check. Next, run tests.');
  });
});

describe('voice task conclusion', () => {
  it('uses the final completed public text instead of an earlier progress update', () => {
    const session = sessionWithItems([
      {
        id: 'progress',
        type: 'text',
        content: '正在检查语音任务链路。',
        status: 'completed',
        isStreaming: false,
      },
      {
        id: 'final',
        type: 'text',
        content: [
          '## 已完成',
          '',
          '用户提出的两个语音问题都已处理。',
          '- 电话弹窗现在会显示收尾简报',
          '- 收尾会保留最终回答的关键结论',
          '- 进展前缀仍保持移除',
          '- 聚焦测试已通过',
          '- 第六条内部实现细节不应进入简报',
        ].join('\n'),
        status: 'completed',
        isStreaming: false,
      },
    ]);

    expect(extractVoiceTaskConclusion(session))
      .toBe('用户提出的两个语音问题都已处理。电话弹窗现在会显示收尾简报。收尾会保留最终回答的关键结论。进展前缀仍保持移除。聚焦测试已通过。');
  });

  it('keeps enough of the final answer to respond to the user while remaining a brief', () => {
    const conclusion = summarizeVoiceTaskConclusion(
      'Final result: Yes, the requested behavior is now supported. The closing text is visible in the call popup. The spoken brief retains the answer and key result. Focused tests pass. Restart the current call before testing. Internal implementation details should not be announced.',
    );

    expect(conclusion)
      .toBe('Yes, the requested behavior is now supported. The closing text is visible in the call popup. The spoken brief retains the answer and key result. Focused tests pass. Restart the current call before testing.');
    expect(conclusion.match(/[.!?]/g)?.length).toBeLessThanOrEqual(5);
    expect(conclusion.length).toBeLessThanOrEqual(320);
  });

  it('starts with the answer instead of source-reading preamble, headings, or parentheses', () => {
    const conclusion = summarizeVoiceTaskConclusion([
      '我已经通读了项目的 README.md 和 README.zh-CN.md（项目自述是最权威的定位来源），下面是整理好的介绍。',
      '',
      '## BitFun 项目介绍',
      '',
      'BitFun 是一个桌面 AI Agent，能把任务变成可打开的应用界面。',
      '它支持编码、办公和桌面执行（包括浏览器、终端与文件系统）。',
      'BitFun 是一个桌面 AI Agent，能把任务变成可打开的应用界面。',
    ].join('\n'));

    expect(conclusion).toBe(
      'BitFun 是一个桌面 AI Agent，能把任务变成可打开的应用界面。'
      + '它支持编码、办公和桌面执行包括浏览器、终端与文件系统。',
    );
    expect(conclusion).not.toMatch(/[()（）]/);
    expect(conclusion).not.toContain('README.md');
    expect(conclusion.match(/BitFun 是一个桌面 AI Agent/g)).toHaveLength(1);
  });

  it('returns an empty conclusion when no final public text exists', () => {
    expect(extractVoiceTaskConclusion(sessionWithItems([]))).toBe('');
  });
});
