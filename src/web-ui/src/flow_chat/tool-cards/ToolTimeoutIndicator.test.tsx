import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { I18nextProvider, initReactI18next } from 'react-i18next';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { beforeAll, describe, expect, it } from 'vitest';
import { ToolTimeoutIndicator } from './ToolTimeoutIndicator';

let i18n: I18nInstance;

beforeAll(async () => {
  i18n = createInstance();
  await i18n.use(initReactI18next).init({
    lng: 'en-US',
    fallbackLng: 'en-US',
    resources: {
      'en-US': {
        'flow-chat': {
          toolCards: {
            timeout: {
              completedDurationTooltip: 'Completed in {{duration}}',
              failedDurationTooltip: 'Failed after {{duration}}',
              failedDurationTooltipWithReason: 'Failed after {{duration}}: {{reason}}',
              cancelledDurationTooltip: 'Cancelled after {{duration}}',
              durationTooltip: 'Duration {{duration}}',
            },
          },
        },
      },
    },
    interpolation: { escapeValue: false },
  });
});

function renderIndicator(element: React.ReactElement): string {
  return renderToStaticMarkup(
    <I18nextProvider i18n={i18n}>
      {element}
    </I18nextProvider>,
  );
}

describe('ToolTimeoutIndicator', () => {
  it('uses a success affordance for completed subagent durations', () => {
    const html = renderIndicator(
      <ToolTimeoutIndicator
        isRunning={false}
        completedDurationMs={1250}
        completedStatus="success"
      />,
    );

    expect(html).toContain('duration-text--completed-success');
    expect(html).toContain('Completed in 1.3s');
    expect(html).toContain('1.3s');
  });

  it('uses an error affordance with the failure reason in the hover text', () => {
    const html = renderIndicator(
      <ToolTimeoutIndicator
        isRunning={false}
        completedDurationMs={2400}
        completedStatus="error"
        completedFailureReason="provider timed out"
      />,
    );

    expect(html).toContain('duration-text--completed-error');
    expect(html).toContain('Failed after 2.4s: provider timed out');
    expect(html).toContain('2.4s');
  });
});
