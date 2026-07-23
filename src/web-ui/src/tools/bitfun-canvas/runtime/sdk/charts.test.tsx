import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { BarChart, LineChart, PieChart } from './charts';

describe('Canvas chart components', () => {
  it('renders bar chart SVG from numeric arrays', () => {
    const html = renderToStaticMarkup(<BarChart title="Builds" data={[2, 4]} />);

    expect(html).toContain('aria-label="Builds"');
    expect(html).toContain('<rect');
    expect(html).toContain('bf-chart');
  });

  it('renders line chart SVG from object rows', () => {
    const html = renderToStaticMarkup(
      <LineChart
        data={[
          { label: 'Mon', value: 1 },
          { label: 'Tue', value: 3 },
        ]}
      />,
    );

    expect(html).toContain('aria-label="Line chart"');
    expect(html).toContain('<path');
    expect(html).toContain('<circle');
  });

  it('renders pie chart slices from series values', () => {
    const html = renderToStaticMarkup(
      <PieChart
        series={[
          { name: 'Used', value: 70 },
          { name: 'Free', value: 30 },
        ]}
      />,
    );

    expect(html).toContain('aria-label="Pie chart"');
    expect(html).toContain('<path');
    expect(html).toContain('70%');
    expect(html).toContain('30%');
  });
});
