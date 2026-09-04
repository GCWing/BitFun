import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { Card, CardBody, CardHeader, Empty, Pill, Tabs } from './adapters';

describe('OpenBitFun Canvas structural adapters', () => {
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
