import { describe, expect, it } from 'vitest';
import type { AiErrorPresentation } from '@/shared/ai-errors/aiErrorPresenter';
import { buildInterruptionDiagnostics } from './interruptionDiagnostics';

const t = (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => {
  const template = options?.defaultValue ?? _key;
  return template.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? _match));
};

const presentation: AiErrorPresentation = {
  category: 'provider_unavailable',
  titleKey: 'errors:ai.providerUnavailable.title',
  messageKey: 'errors:ai.providerUnavailable.message',
  severity: 'warning',
  retryable: true,
  actions: [
    { code: 'wait_and_retry', labelKey: 'errors:ai.actions.waitAndRetry' },
    { code: 'copy_diagnostics', labelKey: 'errors:ai.actions.copyDiagnostics' },
  ],
  diagnostics: '',
};

describe('buildInterruptionDiagnostics', () => {
  it('prefers sanitized diagnostics produced by the AI error presenter', () => {
    expect(buildInterruptionDiagnostics(
      { category: 'network', rawMessage: 'raw' },
      { ...presentation, diagnostics: 'sanitized diagnostics' },
      t,
    )).toBe('sanitized diagnostics');
  });

  it('builds fallback diagnostics without unbounded provider payloads', () => {
    const diagnostics = buildInterruptionDiagnostics(
      {
        category: 'provider_unavailable',
        provider: 'anthropic',
        providerCode: 'overloaded_error',
        providerMessage: 'x'.repeat(520),
        httpStatus: 529,
        requestId: 'req-1',
        rawMessage: 'y'.repeat(520),
      },
      presentation,
      t,
    );

    expect(diagnostics).toContain('=== Deep Review Interruption Diagnostics ===');
    expect(diagnostics).toContain('Error type: provider_unavailable (provider_unavailable)');
    expect(diagnostics).toContain('Suggested actions: wait_and_retry, copy_diagnostics');
    expect(diagnostics).toContain('  - provider: anthropic');
    expect(diagnostics).toContain('  - provider message: ');
    expect(diagnostics).toContain('... [truncated]');
    expect(diagnostics.length).toBeLessThan(1500);
  });

  it('expands terse presenter diagnostics when the interruption has a raw failure message', () => {
    const diagnostics = buildInterruptionDiagnostics(
      {
        category: 'unknown',
        rawMessage: 'Conversation execution failed after ReviewSecurity completed.',
      },
      {
        ...presentation,
        category: 'unknown',
        titleKey: 'errors:ai.executionFailed',
        messageKey: 'errors:ai.genericSuggestion',
        diagnostics: 'category=unknown',
      },
      t,
    );

    expect(diagnostics).toContain('=== Deep Review Interruption Diagnostics ===');
    expect(diagnostics).toContain('Error type: unknown (unknown)');
    expect(diagnostics).toContain('raw message: Conversation execution failed after ReviewSecurity completed.');
    expect(diagnostics).not.toBe('category=unknown');
  });
});
