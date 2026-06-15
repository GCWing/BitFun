import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { RecoveryPlanPreview } from './RecoveryPlanPreview';

const messages: Record<string, string> = {
  'deepReviewActionBar.recoveryPreserve': '{{count}} completed reviewers will be preserved',
  'deepReviewActionBar.recoveryRerun': '{{count}} reviewers will be rerun',
  'deepReviewActionBar.recoverySkip': '{{count}} reviewers will be skipped',
};

function t(key: string, options?: Record<string, unknown> & { defaultValue?: string }): string {
  const template = messages[key] ?? options?.defaultValue ?? key;
  return template.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? _match));
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t,
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
