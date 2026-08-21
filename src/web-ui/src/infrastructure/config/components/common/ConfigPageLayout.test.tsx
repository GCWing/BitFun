// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import {
  ConfigPageLayout,
  ConfigPageContent,
  ConfigPageRow,
  ConfigPageSection,
  ConfigPageSectionStack,
} from './ConfigPageLayout';

describe('ConfigPageLayout', () => {
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

  it('preserves the standard section stack inside a shared wrapper', () => {
    act(() => {
      root.render(
        <ConfigPageContent>
          <ConfigPageSectionStack data-testid="section-stack">
            <ConfigPageSection title="First">
              <div>First body</div>
            </ConfigPageSection>
            <ConfigPageSection title="Second">
              <div>Second body</div>
            </ConfigPageSection>
          </ConfigPageSectionStack>
        </ConfigPageContent>,
      );
    });

    const contentInner = container.querySelector('.bitfun-config-page-content__inner');
    const stack = container.querySelector('[data-testid="section-stack"]');

    expect(contentInner?.children).toHaveLength(1);
    expect(contentInner?.firstElementChild).toBe(stack);
    expect(stack?.classList.contains('bitfun-config-page-section-stack')).toBe(true);
    expect(stack?.querySelectorAll(':scope > .bitfun-config-page-section')).toHaveLength(2);
  });

  it('strips the body surface chrome when the section opts out of the mouse glow', () => {
    act(() => {
      root.render(
        <ConfigPageSection title="Managed" mouseGlowSurface={false}>
          <div>Body</div>
        </ConfigPageSection>,
      );
    });

    const section = container.querySelector('.bitfun-config-page-section');
    const body = container.querySelector('.bitfun-config-page-section__body');

    expect(body?.classList.contains('bitfun-config-page-section__body--flush')).toBe(true);
    // The prop drives styling only; it must never leak onto the DOM node.
    expect(section?.hasAttribute('mouseglowsurface')).toBe(false);
  });

  it('keeps the body surface chrome by default', () => {
    act(() => {
      root.render(
        <ConfigPageSection title="Standard">
          <div>Body</div>
        </ConfigPageSection>,
      );
    });

    const body = container.querySelector('.bitfun-config-page-section__body');
    expect(body?.classList.contains('bitfun-config-page-section__body--flush')).toBe(false);
  });

  it('lets copy use the full row when there is no control', () => {
    act(() => {
      root.render(
        <ConfigPageRow label="User hook file" description="A long platform-specific path">
          {null}
        </ConfigPageRow>,
      );
    });

    const row = container.querySelector('.bitfun-config-page-row');
    expect(row?.classList.contains('bitfun-config-page-row--no-control')).toBe(true);
    expect((row as HTMLElement | null)?.style.gridTemplateColumns).toBe('minmax(0, 1fr)');
    expect(row?.querySelector('.bitfun-config-page-row__control')).toBeNull();
  });

  it('forwards a feature-owned Appearance contract to the real layout nodes', () => {
    act(() => {
      root.render(
        <ConfigPageLayout
          data-testid="feature-root"
          data-bf-component="model-settings"
          data-bf-part="root"
        >
          <ConfigPageContent
            data-testid="feature-content"
            data-bf-component="model-settings"
            data-bf-part="providerSelection"
          >
            <ConfigPageSection
              title="Models"
              data-testid="feature-section"
              data-bf-component="model-settings"
              data-bf-part="providerGroup"
            >
              <div>Body</div>
            </ConfigPageSection>
          </ConfigPageContent>
        </ConfigPageLayout>,
      );
    });

    expect(container.querySelector('[data-testid="feature-root"]')?.getAttribute('data-bf-component')).toBe('model-settings');
    expect(container.querySelector('[data-testid="feature-content"]')?.getAttribute('data-bf-part')).toBe('providerSelection');
    expect(container.querySelector('[data-testid="feature-section"]')?.getAttribute('data-bf-part')).toBe('providerGroup');
  });
});
