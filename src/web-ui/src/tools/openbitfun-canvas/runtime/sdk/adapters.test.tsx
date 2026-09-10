import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { Button, Card, CardBody, CardHeader, Empty, Pill, Tabs } from './adapters';

describe('OpenBitFun Canvas structural adapters', () => {
  it('preserves primary action emphasis, disabled state, and existing secondary choices', () => {
    const primary = renderToStaticMarkup(<Button variant="primary" size="small" disabled>Save</Button>);
    expect(primary).toContain('data-openbitfun-variant="primary"');
    expect(primary).toContain('data-size="sm"');
    expect(primary).toContain('disabled=""');
    for (const variant of ['secondary', 'ghost'] as const) {
      expect(renderToStaticMarkup(<Button variant={variant}>More</Button>))
        .toContain('data-openbitfun-variant="outline"');
    }
  });

  it('composes cards through the design-system anatomy', () => {
    const markup = renderToStaticMarkup(
      <Card variant="elevated" padding="medium">
        <CardHeader title="Build" subtitle="Ready to run" />
        <CardBody>Details</CardBody>
      </Card>,
    );

    expect(markup).toContain('data-openbitfun-component="card"');
    expect(markup).toContain('data-appearance="raised"');
    expect(markup).toContain('data-openbitfun-part="header"');
    expect(markup).toContain('data-openbitfun-part="body"');
  });

  it('maps pills and empty states to stable feedback primitives', () => {
    const markup = renderToStaticMarkup(
      <>
        <Pill tone="warning">Needs attention</Pill>
        <Empty description="Nothing here" />
      </>,
    );

    expect(markup).toContain('data-openbitfun-component="status-pill"');
    expect(markup).toContain('data-tone="warning"');
    expect(markup).toContain('data-openbitfun-component="empty"');
    expect(markup).toContain('Nothing here');
  });

  it('connects tab selection with the matching panel', () => {
    const markup = renderToStaticMarkup(
      <Tabs
        defaultActiveKey="second"
        items={[
          { key: 'first', label: 'First', children: 'First panel' },
          { key: 'second', label: 'Second', children: 'Second panel' },
        ]}
      />,
    );

    expect(markup).toContain('data-openbitfun-component="tab-group"');
    expect(markup).toContain('aria-selected="true"');
    expect(markup).toContain('role="tabpanel"');
    expect(markup).toContain('Second panel');
    expect(markup).not.toContain('First panel');
  });
});
