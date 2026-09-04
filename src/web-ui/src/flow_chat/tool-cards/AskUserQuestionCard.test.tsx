// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';
import {
  LOCAL_SURFACE_ID,
  activateSurface,
} from '@/infrastructure/peer-device/deviceSurface';
import { PeerDeviceContext } from '@/infrastructure/peer-device/peerDeviceContextState';
import { askUserQuestionDraftStore } from '../store/askUserQuestionDraftStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => (
      options?.count === undefined ? key : `${key}:${String(options.count)}`
    ),
  }),
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
    expect(container.querySelector('[data-openbitfun-component="ask-user"] [data-openbitfun-part="body"]')).not.toBeNull();
    expect(container.querySelector('button[data-openbitfun-part="summary"]')).toBeNull();

    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('completed')}
          config={config}
          isLastItem
        />,
      );
    });
    expect(container.querySelector('[data-openbitfun-component="ask-user"] [data-openbitfun-part="body"]')).not.toBeNull();
    expect(container.querySelector('button[data-openbitfun-part="summary"]')).toBeNull();

    act(() => {
      root.render(
        <AskUserQuestionCard
          toolItem={questionTool('completed')}
          config={config}
          isLastItem={false}
        />,
      );
    });
    expect(container.querySelector('button[data-openbitfun-part="summary"]')).not.toBeNull();
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

  it('switches to the draft owned by the newly activated device surface', () => {
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

    const localRadio = container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]');
    act(() => localRadio?.click());
    expect(localRadio?.checked).toBe(true);

    act(() => {
      activateSurface('peer-device-b');
    });

    expect(
      container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]')?.checked,
    ).toBe(false);
  });

  it('explains why an older CLI peer cannot answer instead of exposing a dead form', () => {
    activateSurface('peer-cli');
    act(() => {
      root.render(
        <PeerDeviceContext.Provider value={{
          peerMode: { active: true, deviceId: 'peer-cli', deviceName: 'CLI' },
          attachments: [],
          currentPeerCapabilities: {
            idempotentDialogSubmit: true,
            targetedSessionRollback: true,
            tokenUsageStatistics: true,
            miniAppAgentContextFilesV1: false,
            cancelTool: false,
            toolCatalog: false,
            userQuestionResponse: null,
            hostKind: 'cli',
          },
          switchToDevice: vi.fn(),
          switchToLocal: vi.fn(),
          disconnectDevice: vi.fn(),
          disconnectAllDevices: vi.fn(),
        }}>
          <AskUserQuestionCard
            toolItem={questionTool('pending_confirmation')}
            config={config}
            sessionId="session-a"
            isLastItem
          />
        </PeerDeviceContext.Provider>,
      );
    });

    expect(container.querySelector('[data-openbitfun-component="ask-user"]')?.getAttribute('data-openbitfun-state'))
      .toBe('error');
    expect(container.querySelector('[data-openbitfun-part="status-label"]')?.textContent)
      .toBe('toolCards.askUser.unsupportedOnPeer');
    expect(container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]')?.disabled)
      .toBe(true);
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

    const customInput = container.querySelector<HTMLInputElement>('[data-openbitfun-part="custom-input"] input');
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
    expect(container.querySelector<HTMLInputElement>('[data-openbitfun-part="custom-input"] input')?.value).toBe('CockroachDB');
  });

  it('keeps the custom input mounted and focused during Chinese IME composition', () => {
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

    const otherRadio = container.querySelector<HTMLInputElement>('input[value="Other"]');
    act(() => otherRadio?.click());

    const customInput = container.querySelector<HTMLInputElement>('[data-openbitfun-part="custom-input"] input');
    expect(customInput).not.toBeNull();
    act(() => {
      customInput?.focus();
      customInput?.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      if (customInput) {
        setInputValue(customInput, 'n');
        setInputValue(customInput, '');
      }
    });

    expect(container.querySelector('[data-openbitfun-part="custom-input"] input')).toBe(customInput);
    expect(document.activeElement).toBe(customInput);
    expect(container.querySelector<HTMLInputElement>('input[value="Other"]')?.checked).toBe(true);

    act(() => {
      if (customInput) {
        setInputValue(customInput, '你');
        customInput.dispatchEvent(new CompositionEvent('compositionend', {
          bubbles: true,
          data: '你',
        }));
      }
    });

    expect(container.querySelector<HTMLInputElement>('[data-openbitfun-part="custom-input"] input')?.value).toBe('你');
    expect(document.activeElement).toBe(customInput);
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

    const customInput = container.querySelector<HTMLInputElement>('[data-openbitfun-part="custom-input"] input');
    expect(customInput).not.toBeNull();
    act(() => {
      if (customInput) {
        setInputValue(customInput, 'Custom database');
        setInputValue(customInput, '');
      }
    });

    expect(container.querySelector<HTMLInputElement>('input[value="Other"]')?.checked).toBe(false);
    expect(container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]')?.checked).toBe(true);

    const submitButton = container.querySelector<HTMLButtonElement>('[data-openbitfun-part="submit"] button');
    expect(submitButton?.disabled).toBe(false);
    await act(async () => submitButton?.click());

    expect(toolAPI.submitUserAnswers).toHaveBeenCalledWith(
      'question-tool-1',
      { 0: ['PostgreSQL'] },
      'session-a',
    );
  });

  it('keeps the form retryable and reports a failed response submission', async () => {
    vi.mocked(toolAPI.submitUserAnswers).mockRejectedValueOnce(new Error('peer unavailable'));
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

    act(() => {
      container.querySelector<HTMLInputElement>('input[value="PostgreSQL"]')?.click();
    });
    const submitButton = container.querySelector<HTMLButtonElement>(
      '[data-openbitfun-part="submit"] button',
    );
    await act(async () => submitButton?.click());

    expect(container.querySelector('[data-openbitfun-part="status-label"]')?.textContent)
      .toBe('toolCards.askUser.submitFailed');
    expect(submitButton?.disabled).toBe(false);
  });
});
