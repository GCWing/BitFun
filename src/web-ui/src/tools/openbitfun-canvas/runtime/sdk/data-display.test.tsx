import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
  Alert,
  Callout,
  CollapsibleSection,
  DiffStats,
  DiffView,
  FileTree,
  KeyValueList,
  ProgressBar,
  Swatch,
  Timeline,
  TodoList,
  TodoListCard,
  UsageBar,
} from './data-display';

describe('OpenBitFun Canvas data display components', () => {
  it('renders callouts as lightweight themed notes', () => {
    const markup = renderToStaticMarkup(
      <Callout tone="info" title="Repository Rule">
        Keep product logic platform-agnostic.
      </Callout>,
    );

    expect(markup).toContain('openbitfun-callout');
    expect(markup).toContain('openbitfun-callout-title');
    expect(markup).toContain('openbitfun-callout-body');
    expect(markup).toContain('--openbitfun-callout-accent:var(--openbitfun-canvas-info)');
    expect(markup).toContain('Repository Rule');
  });

  it('renders sandbox-native alerts without host i18n requirements', () => {
    const markup = renderToStaticMarkup(
      <Alert type="warning" title="Risk" message="Check deployment order" />,
    );

    expect(markup).toContain('openbitfun-alert');
    expect(markup).toContain('role="alert"');
    expect(markup).toContain('Risk');
    expect(markup).toContain('Check deployment order');
  });

  it('renders collapsible sections from the bundled SDK', () => {
    const markup = renderToStaticMarkup(
      <CollapsibleSection title="Files" count={2} defaultOpen>
        <span>src/main.rs</span>
      </CollapsibleSection>,
    );

    expect(markup).toContain('openbitfun-collapsible-section');
    expect(markup).toContain('aria-expanded="true"');
    expect(markup).toContain('Files');
    expect(markup).toContain('src/main.rs');
  });

  it('renders key value lists from records', () => {
    const markup = renderToStaticMarkup(
      <KeyValueList
        columns={2}
        items={{
          Status: 'Ready',
          Owner: 'Canvas',
        }}
      />,
    );

    expect(markup).toContain('openbitfun-key-value-list');
    expect(markup).toContain('Status');
    expect(markup).toContain('Ready');
    expect(markup).toContain('repeat(2, minmax(0, 1fr))');
  });

  it('renders timeline events with time and description', () => {
    const markup = renderToStaticMarkup(
      <Timeline
        items={[
          { title: 'Compiled', description: 'Runtime bundle loaded', time: '10:00', tone: 'success' },
        ]}
      />,
    );

    expect(markup).toContain('openbitfun-timeline');
    expect(markup).toContain('Compiled');
    expect(markup).toContain('Runtime bundle loaded');
    expect(markup).toContain('<time');
  });

  it('renders nested file trees', () => {
    const markup = renderToStaticMarkup(
      <FileTree
        items={[
          {
            name: 'src',
            type: 'folder',
            children: [{ name: 'main.rs', meta: '+12' }],
          },
        ]}
      />,
    );

    expect(markup).toContain('openbitfun-file-tree');
    expect(markup).toContain('<details open="">');
    expect(markup).toContain('main.rs');
    expect(markup).toContain('+12');
  });

  it('uses dedicated code-change colors for diff counts and lines', () => {
    const markup = renderToStaticMarkup(
      <>
        <DiffStats additions={12} deletions={3} />
        <DiffView lines={['+added', '-removed']} />
      </>,
    );

    expect(markup).toContain('--openbitfun-color-code-change-added');
    expect(markup).toContain('--openbitfun-color-code-change-removed');
    expect(markup).not.toContain('--openbitfun-color-status-success-content');
    expect(markup).not.toContain('--openbitfun-color-status-danger-content');
  });

  it('renders progress bars with bounded values', () => {
    const markup = renderToStaticMarkup(
      <ProgressBar value={125} max={100} label="Coverage" tone="success" />,
    );

    expect(markup).toContain('openbitfun-progress');
    expect(markup).toContain('role="progressbar"');
    expect(markup).toContain('aria-valuenow="100"');
    expect(markup).toContain('100%');
  });

  it('renders swatches and usage bars from palette tokens', () => {
    const markup = renderToStaticMarkup(
      <>
        <Swatch color="purple" title="Frontend" />
        <UsageBar
          topLeftLabel="Context"
          topRightLabel="75%"
          total={100}
          segments={[
            { id: 'input', value: 40, color: 'blue' },
            { id: 'output', value: 35, color: 'green' },
          ]}
        />
      </>,
    );

    expect(markup).toContain('openbitfun-swatch');
    expect(markup).toContain('openbitfun-usage-bar');
    expect(markup).toContain('role="progressbar"');
    expect(markup).toContain('aria-valuenow="75"');
    expect(markup).toContain('Context');
  });

  it('renders todo lists and cards without host services', () => {
    const todos = [
      { id: 'one', content: 'Map SDK surface', status: 'completed' as const },
      { id: 'two', content: 'Align docs skill', status: 'in_progress' as const },
    ];

    const markup = renderToStaticMarkup(
      <>
        <TodoList todos={todos} dimmedTodoIds={['one']} />
        <TodoListCard todos={todos} defaultExpanded />
      </>,
    );

    expect(markup).toContain('openbitfun-todo-list');
    expect(markup).toContain('openbitfun-todo-list-card');
    expect(markup).toContain('Map SDK surface');
    expect(markup).toContain('1/2 done');
    expect(markup).toContain('aria-expanded="true"');
  });
});
