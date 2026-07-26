// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import {
  ConfigPageContent,
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
});
