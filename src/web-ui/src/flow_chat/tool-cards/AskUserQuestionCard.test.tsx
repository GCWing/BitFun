// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => (
      options?.count === undefined ? key : `${key}:${String(options.count)}`
    ),
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    isLoading: _isLoading,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & { isLoading?: boolean }) => (
    <button type="button" {...props}>{children}</button>
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/api/service-api/ToolAPI', () => ({
  toolAPI: {
    submitUserAnswers: vi.fn(),
  },
}));

import { AskUserQuestionCard } from './AskUserQuestionCard';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const config: ToolCardConfig = {
  toolName: 'AskUserQuestion',
  displayName: 'Ask User',
  icon: 'Q',
  requiresConfirmation: false,
  resultDisplayType: 'detailed',
};

function questionTool(status: FlowToolItem['status']): FlowToolItem {
  return {
    id: 'question-tool-1',
    type: 'tool',
    toolName: 'AskUserQuestion',
    timestamp: 1,
    status,
    toolCall: {
      id: 'question-call-1',
      input: {
        questions: [{
          header: 'Database',
          question: 'Which database?',
          multiSelect: false,
          options: [{
            label: 'PostgreSQL',
            description: 'Use PostgreSQL',
          }],
        }],
      },
    },
    ...(status === 'completed'
      ? {
          toolResult: {
            success: true,
            result: {
              answers: {
                0: 'PostgreSQL',
              },
            },
          },
        }
      : {}),
  };
}

describe('AskUserQuestionCard', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('keeps a just-completed tail question visible until newer content arrives', () => {
    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('pending_confirmation')}
          config={config}
          isLastItem
        />,
      );
    });
    expect(container.querySelector('.questions-container')).not.toBeNull();
    expect(container.querySelector('.completed-summary')).toBeNull();

    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('completed')}
          config={config}
          isLastItem
        />,
      );
    });
    expect(container.querySelector('.questions-container')).not.toBeNull();
    expect(container.querySelector('.completed-summary')).toBeNull();

    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('completed')}
          config={config}
          isLastItem={false}
        />,
      );
    });
    expect(container.querySelector('.completed-summary')).not.toBeNull();
  });
});
