// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Modal, ModalProvider } from '@bitfun/ui';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { Card, CardBody, CardFooter, CardHeader } from './Card/Card';

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

describe('component appearance contracts', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.getElementById('bitfun-appearance-overlay-host')?.remove();
    container.remove();
  });

  it('exposes stable part, facet, and state attributes for base components', () => {
    const card = renderToStaticMarkup(
      <Card variant="accent" padding="large" interactive>
        <CardHeader title="Title" subtitle="Subtitle" />
        <CardBody>Body</CardBody>
        <CardFooter align="between">Footer</CardFooter>
      </Card>,
    );
    expect(card).toContain('data-bf-component="card"');
    expect(card).toContain('data-bf-part="title"');
    expect(card).toContain('data-bf-align="between"');
    expect(card).toContain('data-bf-state="interactive"');
  });

  it('renders Modal through the shared overlay host with stable parts', async () => {
    await act(async () => {
      root.render(
        <ModalProvider portalContainer={getAppearanceOverlayHost}>
          <Modal isOpen onClose={vi.fn()} title="Contract" size="large" contentPadding="lg" resizable>
            Content
          </Modal>
        </ModalProvider>,
      );
    });
    const host = document.getElementById('bitfun-appearance-overlay-host');
    expect(host).not.toBeNull();
    expect(host?.querySelector('[data-bf-component="modal"][data-bf-part="overlay"]')).not.toBeNull();
    expect(host?.querySelector('[data-bf-part="dialog"][data-bf-size="large"]')).not.toBeNull();
    expect(host?.querySelector('[data-bf-part="content"][data-bf-padding="lg"]')).not.toBeNull();
    expect(host?.querySelectorAll('[data-bf-part="resizeHandle"]')).toHaveLength(8);
  });
});
