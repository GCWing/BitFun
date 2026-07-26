import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';

import { DependencyGraph, FlowDiagram } from './diagrams';
import { computeDAGLayout } from './diagramLayout';

describe('Canvas diagram helpers', () => {
  it('computes a vertical DAG layout compatible with generated architecture canvases', () => {
    const layout = computeDAGLayout({
      nodes: [{ id: 'ui' }, { id: 'runtime' }, { id: 'sdk' }],
      edges: [
        { from: 'ui', to: 'runtime' },
        { from: 'runtime', to: 'sdk' },
      ],
      nodeWidth: 100,
      nodeHeight: 30,
      rankGap: 20,
      padding: 10,
    });

    expect(layout.direction).toBe('vertical');
    expect(layout.width).toBe(120);
    expect(layout.height).toBe(150);
    expect(layout.nodes.map(node => [node.id, node.rank, node.x, node.y])).toEqual([
      ['ui', 0, 10, 10],
      ['runtime', 1, 10, 60],
      ['sdk', 2, 10, 110],
    ]);
    expect([...layout].map(node => node.id)).toEqual(['ui', 'runtime', 'sdk']);
    expect(layout.find(node => node.id === 'runtime')?.y).toBe(60);
    expect(layout.map(node => node.id)).toEqual(['ui', 'runtime', 'sdk']);
    expect(layout.edges[0]).toMatchObject({
      from: 'ui',
      to: 'runtime',
      sourceX: 60,
      targetX: 60,
    });
  });

  it('computes a horizontal DAG layout', () => {
    const layout = computeDAGLayout({
      direction: 'horizontal',
      nodes: [{ id: 'a' }, { id: 'b' }],
      edges: [{ from: 'a', to: 'b' }],
      nodeWidth: 80,
      nodeHeight: 32,
      rankGap: 24,
      padding: 8,
    });

    expect(layout.direction).toBe('horizontal');
    expect(layout.width).toBe(200);
    expect(layout.height).toBe(48);
    expect(layout.edges[0]).toMatchObject({
      sourceX: 88,
      sourceY: 24,
      targetX: 112,
      targetY: 24,
    });
  });

  it('accepts Cursor-style source target edges and preserves node metadata', () => {
    const layout = computeDAGLayout({
      nodes: [
        { id: 'entrypoints', label: 'Entrypoints', subtitle: 'Desktop · CLI', group: 'app' },
        { id: 'assembly', label: 'Assembly', subtitle: 'bitfun-core', group: 'assembly' },
      ],
      edges: [{ source: 'entrypoints', target: 'assembly' }],
      nodeWidth: 120,
      nodeHeight: 48,
      padding: 12,
    });

    expect(layout.nodes[0]).toMatchObject({
      id: 'entrypoints',
      label: 'Entrypoints',
      subtitle: 'Desktop · CLI',
      group: 'app',
      x: 12,
      y: 12,
      centerX: 72,
      centerY: 36,
      width: 120,
      height: 48,
    });
    expect(layout.nodes[0].meta).toMatchObject({
      id: 'entrypoints',
      label: 'Entrypoints',
      subtitle: 'Desktop · CLI',
      group: 'app',
    });
    expect(layout.edges[0]).toMatchObject({
      from: 'entrypoints',
      to: 'assembly',
    });
    expect(layout.edges[0].path).toContain('M ');
    expect(layout.ranks[0].nodeIds).toEqual(['entrypoints']);
    expect(layout.ranks[0].nodes?.[0]).toMatchObject({ id: 'entrypoints', label: 'Entrypoints' });
  });

  it('keeps custom node metadata available for generated SVG maps', () => {
    const layout = computeDAGLayout({
      nodes: [
        { id: 'interfaces', label: 'Interfaces', modules: ['desktop', 'cli'] },
        { id: 'contracts', label: 'Contracts', modules: ['events'] },
      ],
      edges: [{ from: 'interfaces', to: 'contracts' }],
      nodeWidth: 100,
      nodeHeight: 40,
      padding: 10,
    });

    expect(layout.nodes[0].x).toBe(10);
    expect(layout.nodes[0].centerX).toBe(60);
    expect(layout.nodes[0].modules).toEqual(['desktop', 'cli']);
    expect(layout.nodes[0].meta?.modules).toEqual(['desktop', 'cli']);
    expect(layout.nodes[0].source?.modules).toEqual(['desktop', 'cli']);
  });

  it('renders a dependency graph from layout nodes and edges', () => {
    const markup = renderToStaticMarkup(
      <DependencyGraph
        title="Runtime"
        nodes={[
          { id: 'shell', label: 'Shell', tone: 'info' },
          { id: 'sdk', label: 'SDK', description: 'Adapters', tone: 'success' },
        ]}
        edges={[{ from: 'shell', to: 'sdk', label: 'injects' }]}
      />,
    );

    expect(markup).toContain('bf-diagram');
    expect(markup).toContain('aria-label="Runtime"');
    expect(markup).toContain('var(--element-bg-subtle)');
    expect(markup).toContain('<path');
    expect(markup).toContain('width="4"');
    expect(markup).toContain('Shell');
    expect(markup).toContain('Adapters');
  });

  it('renders flow diagrams from ordered steps', () => {
    const markup = renderToStaticMarkup(
      <FlowDiagram
        steps={[
          'Compile',
          { title: 'Bundle', description: 'Runtime assets' },
          'Render',
        ]}
      />,
    );

    expect(markup).toContain('aria-label="Flow diagram"');
    expect(markup).toContain('Compile');
    expect(markup).toContain('Runtime assets');
    expect(markup).toContain('Render');
  });
});
