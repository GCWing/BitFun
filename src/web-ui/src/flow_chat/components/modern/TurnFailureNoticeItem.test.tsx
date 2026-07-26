// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TurnFailureNoticeItem } from './TurnFailureNoticeItem';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const TRANSLATIONS: Record<string, string> = {
  'errors:ai.authError': 'API authentication failed',
  'errors:ai.authErrorSuggestion': 'The API key is invalid or expired. Check the model configuration.',
  'turnFailure.showDetails': 'Show technical details',
  'turnFailure.hideDetails': 'Hide technical details',
  'turnFailure.provider': 'Provider',
  'turnFailure.errorCode': 'Error code',
  'turnFailure.httpStatus': 'HTTP status',
  'turnFailure.requestId': 'Request ID',
  'turnFailure.providerError': 'Provider error',
  'turnFailure.copy': 'Copy error',
  'turnFailure.copied': 'Copied',
};

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => TRANSLATIONS[key] ?? key,
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ content, children }: { content: string; children: React.ReactElement }) => (
    <span data-tooltip={content}>{children}</span>
  ),
}));

describe('TurnFailureNoticeItem', () => {
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
    vi.restoreAllMocks();
  });

  it('keeps the failure summary on one row and exposes details through an icon button', () => {
    act(() => {
      root.render(
        <TurnFailureNoticeItem
          error="Invalid API key"
          errorDetail={{ category: 'auth', rawMessage: 'Invalid API key' }}
        />,
      );
    });

    const header = container.querySelector('.turn-failure-notice__header');
    const summary = container.querySelector('.turn-failure-notice__summary');
    const toggle = container.querySelector<HTMLButtonElement>('.turn-failure-notice__details-toggle');
    expect(header).not.toBeNull();
    expect(summary?.textContent).toBe(
      'API authentication failedThe API key is invalid or expired. Check the model configuration.',
    );
    expect(toggle?.textContent).toBe('');
    expect(toggle?.getAttribute('aria-label')).toBe('Show technical details');
    expect(toggle?.getAttribute('aria-expanded')).toBe('false');
    expect(container.textContent).not.toContain('Provider error');
  });

  it('expands raw diagnostics and copies the original error', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    await act(async () => {
      root.render(
        <TurnFailureNoticeItem
          error="Invalid API key"
          errorDetail={{
            category: 'auth',
            provider: 'OpenAI',
            httpStatus: 401,
            rawMessage: 'Invalid API key',
          }}
        />,
      );
    });

    const toggle = container.querySelector<HTMLButtonElement>('.turn-failure-notice__details-toggle');
    act(() => toggle?.click());

    expect(toggle?.getAttribute('aria-expanded')).toBe('true');
    expect(toggle?.getAttribute('aria-label')).toBe('Hide technical details');
    expect(container.querySelector('.turn-failure-notice__raw-error pre')?.textContent).toBe('Invalid API key');

    const copyButton = container.querySelector<HTMLButtonElement>('.turn-failure-notice__copy');
    await act(async () => {
      copyButton?.click();
      await Promise.resolve();
    });

    expect(writeText).toHaveBeenCalledWith('Invalid API key');
  });
});
