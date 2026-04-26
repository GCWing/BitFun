import { describe, expect, it } from 'vitest';
import {
  getAiErrorPresentation,
  normalizeAiErrorDetail,
  type AiErrorDetail,
} from './aiErrorPresenter';

describe('aiErrorPresenter', () => {
  it('classifies GLM 1113 quota errors with actionable recovery', () => {
    const detail = normalizeAiErrorDetail({
      category: 'provider_quota',
      provider: 'glm',
      providerCode: '1113',
      providerMessage: '余额不足或无可用资源包,请充值。',
      requestId: '20260425142416',
    });

    const presentation = getAiErrorPresentation(detail);

    expect(presentation.titleKey).toBe('errors:ai.providerQuota.title');
    expect(presentation.actions.map((action) => action.code)).toEqual([
      'open_model_settings',
      'switch_model',
      'copy_diagnostics',
    ]);
    expect(presentation.retryable).toBe(false);
  });

  it('maps provider-specific balance errors to the same public category', () => {
    const cases: AiErrorDetail[] = [
      { category: 'unknown', provider: 'openai', providerCode: 'insufficient_quota' },
      { category: 'unknown', provider: 'deepseek', httpStatus: 402, providerMessage: 'Insufficient Balance' },
      { category: 'unknown', provider: 'kimi', httpStatus: 403, providerCode: 'NOT_ENOUGH_BALANCE' },
    ];

    for (const value of cases) {
      const presentation = getAiErrorPresentation(normalizeAiErrorDetail(value));
      expect(presentation.category).toBe('provider_quota');
      expect(presentation.actions.some((action) => action.code === 'switch_model')).toBe(true);
    }
  });

  it('returns translation keys from the errors namespace for notification copy', () => {
    const cases: Array<{ detail: AiErrorDetail; titleKey: string; messageKey: string }> = [
      {
        detail: { category: 'network', rawMessage: 'stream closed before response completed' },
        titleKey: 'errors:ai.networkError',
        messageKey: 'errors:ai.networkErrorSuggestion',
      },
      {
        detail: { category: 'rate_limit', httpStatus: 429, rawMessage: 'too many requests' },
        titleKey: 'errors:ai.rateLimit',
        messageKey: 'errors:ai.rateLimitSuggestion',
      },
      {
        detail: { category: 'invalid_request', httpStatus: 400, rawMessage: 'invalid request' },
        titleKey: 'errors:ai.invalidRequest.title',
        messageKey: 'errors:ai.invalidRequest.message',
      },
    ];

    for (const { detail, titleKey, messageKey } of cases) {
      const presentation = getAiErrorPresentation(normalizeAiErrorDetail(detail));
      expect(presentation.titleKey).toBe(titleKey);
      expect(presentation.messageKey).toBe(messageKey);
      expect(presentation.actions.some((action) => action.labelKey === 'errors:ai.actions.copyDiagnostics')).toBe(true);
    }
  });

  it('keeps overloaded provider errors retryable without exposing raw payloads', () => {
    const detail = normalizeAiErrorDetail({
      category: 'unknown',
      provider: 'anthropic',
      httpStatus: 529,
      providerCode: 'overloaded_error',
      providerMessage: 'Anthropic API is temporarily overloaded',
      rawMessage: '{"api_key":"secret","message":"Anthropic API is temporarily overloaded"}',
    });

    const presentation = getAiErrorPresentation(detail);

    expect(presentation.category).toBe('provider_unavailable');
    expect(presentation.retryable).toBe(true);
    expect(presentation.actions.map((action) => action.code)).toContain('wait_and_retry');
    expect(presentation.diagnostics).not.toContain('secret');
  });
});
