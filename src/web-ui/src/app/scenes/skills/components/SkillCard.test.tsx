// @vitest-environment jsdom

import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import SkillCard from './SkillCard';

describe('SkillCard Appearance contract', () => {
  it('exposes variant, meta, action tone, and disabled state', () => {
    const html = renderToStaticMarkup(
      <SkillCard
        name="Canvas"
        iconKind="market"
        meta={<span>12</span>}
        actions={[{
          id: 'download',
          icon: <span>Download</span>,
          ariaLabel: 'Download',
          tone: 'primary',
          disabled: true,
          onClick: () => undefined,
        }]}
      />,
    );

    expect(html).toContain('data-bf-variant="market"');
    expect(html).toContain('data-bf-part="meta"');
    expect(html).toContain('data-bf-part="action"');
    expect(html).toContain('data-bf-tone="primary"');
    expect(html).toContain('data-bf-state="disabled"');
  });
});
