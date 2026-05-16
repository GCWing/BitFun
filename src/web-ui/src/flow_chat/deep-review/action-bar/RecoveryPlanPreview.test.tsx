import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { RecoveryPlanPreview } from './RecoveryPlanPreview';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => {
      const template = options?.defaultValue ?? _key;
      return template.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? _match));
    },
  }),
}));

describe('RecoveryPlanPreview', () => {
  it('renders preserve, rerun, and skip recovery counts', () => {
    const html = renderToStaticMarkup(
      <RecoveryPlanPreview
        recoveryPlan={{
          willPreserve: ['ReviewSecurity', 'ReviewArchitecture'],
          willRerun: ['ReviewPerformance'],
          willSkip: ['ReviewFrontend'],
          summaryText: 'Recovery summary',
        }}
      />,
    );

    expect(html).toContain('2 completed reviewers will be preserved');
    expect(html).toContain('1 reviewers will be rerun');
    expect(html).toContain('1 reviewers will be skipped');
    expect(html).toContain('deep-review-action-bar__recovery-plan');
  });
});
