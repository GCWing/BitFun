// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';
import {
  LOCAL_SURFACE_ID,
  activateSurface,
} from '@/infrastructure/peer-device/deviceSurface';
import { askUserQuestionDraftStore } from '../store/askUserQuestionDraftStore';

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

import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { AskUserQuestionCard } from './AskUserQuestionCard';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const config: ToolCardConfig = {
  toolName: 'AskUserQuestion',
  displayName: 'Ask User',
  icon: 'Q',
  requiresConfirmation: false,
  resultDisplayType: 'detailed',
};

function questionTool(
  status: FlowToolItem['status'],
  multiSelect = false,
): FlowToolItem {
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
          multiSelect,
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

function setInputValue(input: HTMLInputElement, value: string): void {
  const valueSetter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    'value',
  )?.set;
  valueSetter?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('AskUserQuestionCard', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    activateSurface(LOCAL_SURFACE_ID);
    askUserQuestionDraftStore.setState({ drafts: {} });
    vi.mocked(toolAPI.submitUserAnswers).mockReset();
    vi.mocked(toolAPI.submitUserAnswers).mockResolvedValue(undefined);
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

  it('restores an unsubmitted answer after the session card is remounted', () => {
    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('pending_confirmation')}
          config={config}
          sessionId="session-a"
          isLastItem
        />,
      );
    });

    const radio = container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]');
    expect(radio).not.toBeNull();
    act(() => radio?.click());
    expect(radio?.checked).toBe(true);

    act(() => root.render(null));
    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('pending_confirmation')}
          config={config}
          sessionId="session-b"
          isLastItem
        />,
      );
    });
    expect(
      container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]')?.checked,
    ).toBe(false);

    act(() => root.render(null));
    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('pending_confirmation')}
          config={config}
          sessionId="session-a"
          isLastItem
        />,
      );
    });
    expect(
      container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]')?.checked,
    ).toBe(true);
  });

  it('restores an unsubmitted custom input after the card is remounted', () => {
    const renderCard = () => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('pending_confirmation')}
          config={config}
          sessionId="session-a"
          isLastItem
        />,
      );
    };

    act(renderCard);
    const otherRadio = container.querySelector<HTMLInputElement>('input[value="Other"]');
    expect(otherRadio).not.toBeNull();
    act(() => otherRadio?.click());

    const customInput = container.querySelector<HTMLInputElement>('.other-input-inline');
    expect(customInput).not.toBeNull();
    act(() => {
      if (customInput) {
        setInputValue(customInput, 'CockroachDB');
      }
    });
    expect(customInput?.value).toBe('CockroachDB');

    act(() => root.render(null));
    act(renderCard);

    expect(container.querySelector<HTMLInputElement>('input[value="Other"]')?.checked).toBe(true);
    expect(container.querySelector<HTMLInputElement>('.other-input-inline')?.value).toBe('CockroachDB');
  });

  it('deselects a blank multi-select Other answer and omits it from submission', async () => {
    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('pending_confirmation', true)}
          config={config}
          sessionId="session-a"
          isLastItem
        />,
      );
    });

    const databaseCheckbox = container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]');
    const otherCheckbox = container.querySelector<HTMLInputElement>('input[value="Other"]');
    act(() => {
      databaseCheckbox?.click();
      otherCheckbox?.click();
    });

    const customInput = container.querySelector<HTMLInputElement>('.other-input-inline');
    expect(customInput).not.toBeNull();
    act(() => {
      if (customInput) {
        setInputValue(customInput, 'Custom database');
        setInputValue(customInput, '');
      }
    });

    expect(container.querySelector<HTMLInputElement>('input[value="Other"]')?.checked).toBe(false);
    expect(container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]')?.checked).toBe(true);

    const submitButton = container.querySelector<HTMLButtonElement>('.submit-button');
    expect(submitButton?.disabled).toBe(false);
    await act(async () => submitButton?.click());

    expect(toolAPI.submitUserAnswers).toHaveBeenCalledWith(
      'question-tool-1',
      { 0: ['PostgreSQL'] },
    );
  });
});
