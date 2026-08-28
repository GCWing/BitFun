import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import type { FlowToolItem } from '../types/flow-chat';
import { WritePlanDisplay } from './WritePlanDisplay';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  planDisplayProps: [] as Array<Record<string, unknown>>,
}));

vi.mock('./CreatePlanDisplay', () => ({
  PlanDisplay: (props: Record<string, unknown>) => {
    mocks.planDisplayProps.push(props);
    return <div data-testid="plan-display" />;
  },
}));

describe('WritePlanDisplay', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>');
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
    mocks.planDisplayProps = [];
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    vi.unstubAllGlobals();
  });

  it('marks Write plans as project files and preserves their workspace scope', async () => {
    const toolItem = {
      id: 'write-plan',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <WritePlanDisplay
          toolItem={toolItem}
          planFilePath="/workspace/.bitfun/plans/test.plan.md"
          initialContent="---\nname: Test\noverview: Test plan.\ntodos: []\n---\n\n# Test"
          workspacePath="/workspace"
          remoteConnectionId="remote-1"
        />,
      );
    });

    expect(container.querySelector('[data-testid="plan-display"]')).not.toBeNull();
    expect(mocks.planDisplayProps[0]).toMatchObject({
      toolName: 'Write',
      storageKind: 'project-file',
      workspacePath: '/workspace',
      remoteConnectionId: 'remote-1',
    });
  });
});
