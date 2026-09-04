// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ExternalAppsOverview } from './ExternalAppsOverview';
import type { ExternalApplicationView } from './applicationModel';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const application: ExternalApplicationView = {
  ecosystemId: 'opencode',
  displayName: 'OpenCode',
  enabled: true,
  attentionCount: 0,
};

describe('ExternalAppsOverview', () => {
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

  function render(
    applications: ExternalApplicationView[],
    overrides: Partial<React.ComponentProps<typeof ExternalAppsOverview>> = {},
  ) {
    return act(async () => {
      root.render(
        <ExternalAppsOverview
          applications={applications}
          t={((key: string) => key) as never}
          busy={false}
          canMutate
          policiesEnabled
          onToggle={vi.fn()}
          onOpenAttention={vi.fn()}
          onOpenPolicy={vi.fn()}
          {...overrides}
        />,
      );
    });
  }

  it('keeps an application row to its name and switch', async () => {
    await render([application]);

    expect(container.textContent).toContain('OpenCode');
    expect(container.querySelector('[data-openbitfun-part="applicationToggle"] input')).not.toBeNull();
    expect(container.querySelector('.openbitfun-external-sources-config__app-status')).toBeNull();
    expect(container.querySelector('.openbitfun-external-sources-config__app-expand')).toBeNull();
    expect(container.querySelector('.openbitfun-external-sources-config__app-capability-chip')).toBeNull();
  });

  it('opens existing owner settings from an icon-only permission hint', async () => {
    const onOpenAttention = vi.fn();
    await render([{ ...application, attentionCount: 2 }], { onOpenAttention });

    const permissionHint = container.querySelector<HTMLButtonElement>(
      '[data-openbitfun-part="appAttention"]',
    );
    expect(permissionHint?.tagName).toBe('BUTTON');
    expect(permissionHint?.textContent).toBe('');
    expect(permissionHint?.getAttribute('aria-label')).toBe('applications.openAdvanced');
    await act(async () => permissionHint?.click());
    expect(onOpenAttention).toHaveBeenCalledWith('opencode');
  });

  it('applies the switch directly without an intermediate review or confirmation flow', async () => {
    const onToggle = vi.fn();
    await render([application], { onToggle });

    const toggle = container.querySelector<HTMLInputElement>(
      '[data-openbitfun-part="applicationToggle"] input',
    );
    await act(async () => toggle?.click());
    expect(onToggle).toHaveBeenCalledWith(application, false);
  });

  it('explains how to enable a switch disabled by the master setting', async () => {
    const onOpenPolicy = vi.fn();
    await render([application], { policiesEnabled: false, onOpenPolicy });

    const disabledToggle = container.querySelector<HTMLElement>(
      '[data-openbitfun-part="applicationToggle"]',
    );
    expect(disabledToggle?.getAttribute('title')).toBe('applications.enableInAdvanced');
    expect(disabledToggle?.getAttribute('aria-label')).toBe('applications.enableInAdvanced');
    expect(disabledToggle?.tabIndex).toBe(0);
    await act(async () => disabledToggle?.click());
    expect(onOpenPolicy).toHaveBeenCalledOnce();
  });

  it('shows a short neutral empty state when no compatible app settings were found', async () => {
    await render([]);

    expect(container.textContent).toContain('applications.empty');
    expect(container.querySelector('button')).toBeNull();
  });
});
