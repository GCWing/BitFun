import { describe, expect, it } from 'vitest';
import type { FlowTextItem, FlowToolItem } from '../../types/flow-chat';
import {
  shouldReserveProcessingIndicatorSpace,
  shouldShowProcessingIndicator,
} from './processingIndicatorVisibility';

function runtimeStatusItem(): FlowTextItem {
  return {
    id: 'runtime-status-main-round-1',
    type: 'text',
    content: '\u200B',
    timestamp: 1000,
    status: 'streaming',
    isStreaming: true,
    isMarkdown: false,
    runtimeStatus: {
      phase: 'waiting_model',
      scope: 'main',
    },
  };
}

describe('processingIndicatorVisibility', () => {
  it('hides and does not reserve the footer processing indicator when inline runtime status is visible', () => {
    const input = {
      isTurnProcessing: true,
      isSessionProcessing: true,
      processingPhase: 'thinking',
      lastItem: runtimeStatusItem(),
      isContentGrowing: false,
    };

    expect(shouldShowProcessingIndicator(input)).toBe(false);
    expect(shouldReserveProcessingIndicatorSpace(input)).toBe(false);
  });

  it('keeps existing behavior for idle text waits without inline runtime status', () => {
    const input = {
      isTurnProcessing: true,
      isSessionProcessing: false,
      processingPhase: 'thinking',
      lastItem: {
        id: 'answer-1',
        type: 'text',
        content: 'Partial answer',
        timestamp: 1000,
        status: 'streaming',
        isStreaming: true,
      } satisfies FlowTextItem,
      isContentGrowing: false,
    };

    expect(shouldShowProcessingIndicator(input)).toBe(true);
    expect(shouldReserveProcessingIndicatorSpace(input)).toBe(true);
  });

  it('keeps hiding the footer indicator while a tool card is already running', () => {
    const input = {
      isTurnProcessing: true,
      isSessionProcessing: true,
      lastItem: {
        id: 'tool-1',
        type: 'tool',
        toolName: 'Shell',
        toolCall: { input: {}, id: 'tool-1' },
        timestamp: 1000,
        status: 'running',
      } satisfies FlowToolItem,
      isContentGrowing: false,
    };

    expect(shouldShowProcessingIndicator(input)).toBe(false);
    expect(shouldReserveProcessingIndicatorSpace(input)).toBe(true);
  });
});
