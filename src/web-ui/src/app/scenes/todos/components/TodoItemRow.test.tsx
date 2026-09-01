// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CronJob } from '@/infrastructure/api';
import TodoItemRow from './TodoItemRow';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
    formatDate: (date: Date) => date.toISOString(),
  }),
}));

vi.mock('@bitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  Tooltip: ({ children }: { children: React.ReactElement }) => children,
}));

const job: CronJob = {
  id: 'cron_test',
  name: 'Review changes',
  schedule: { kind: 'cron', expr: '0 9 * * *' },
  payload: { text: 'Review the latest changes' },
  enabled: true,
  target: {
    kind: 'workspace',
    workspace: { workspacePath: '/workspace/bitfun' },
    launch: { agentType: 'agentic' },
  },
  createdAtMs: 0,
  configUpdatedAtMs: 0,
  updatedAtMs: 0,
  state: {
    consecutiveFailures: 0,
    coalescedRunCount: 0,
  },
};

describe('TodoItemRow actions', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('uses accessible icon actions and keeps their callbacks isolated from the row', () => {
    const onEdit = vi.fn();
    const onDelete = vi.fn();

    act(() => {
      root.render(
        <TodoItemRow
          job={job}
          atMs={null}
          nowMs={0}
          workspaces={[]}
          statusLabel="Paused"
          onEdit={onEdit}
          onDelete={onDelete}
          onToggleEnabled={vi.fn()}
        />,
      );
    });

    const editButton = container.querySelector<HTMLButtonElement>('button[aria-label="actions.edit"]');
    const deleteButton = container.querySelector<HTMLButtonElement>('button[aria-label="actions.delete"]');

    expect(editButton?.dataset.bfComponent).toBe('icon-button');
    expect(editButton?.dataset.size).toBe('sm');
    expect(deleteButton?.dataset.bfTone).toBe('danger');

    act(() => editButton?.click());
    expect(onEdit).toHaveBeenCalledTimes(1);

    act(() => deleteButton?.click());
    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onEdit).toHaveBeenCalledTimes(1);
  });
});
