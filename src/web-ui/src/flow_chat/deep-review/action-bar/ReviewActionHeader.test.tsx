import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ReviewActionHeader } from './ReviewActionHeader';

const exportActionsMock = vi.hoisted(() => vi.fn(() => <span>export actions</span>));

vi.mock('../../tool-cards/CodeReviewReportExportActions', () => ({
  CodeReviewReportExportActions: exportActionsMock,
}));

describe('ReviewActionHeader', () => {
  beforeEach(() => {
    exportActionsMock.mockClear();
  });

  it('renders export actions, status, error, and minimize control', () => {
    const Icon = () => <span>phase icon</span>;
    const html = renderToStaticMarkup(
      <ReviewActionHeader
        reviewData={{ summary: { recommended_action: 'request_changes' } } as any}
        PhaseIcon={Icon}
        phaseIconClass="phase-class"
        phaseTitle="Review completed"
        errorMessage="Network warning"
        minimizeLabel="Minimize"
        onMinimize={vi.fn()}
      />,
    );

    expect(html).toContain('export actions');
    expect(html).toContain('Review completed');
    expect(html).toContain('Network warning');
    expect(html).toContain('aria-label="Minimize"');
  });

  it('keeps only compact export actions in the top-right controls', () => {
    const Icon = () => <span>phase icon</span>;
    renderToStaticMarkup(
      <ReviewActionHeader
        reviewData={{ summary: { recommended_action: 'request_changes' } } as any}
        PhaseIcon={Icon}
        phaseIconClass="phase-class"
        phaseTitle="Review completed"
        minimizeLabel="Minimize"
        onMinimize={vi.fn()}
      />,
    );

    expect(exportActionsMock.mock.calls[0][0]).toEqual(
      expect.objectContaining({ actions: ['copy', 'save'] }),
    );
  });
});
