/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ReasoningPresetSelector } from './ReasoningPresetSelector';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

describe('ReasoningPresetSelector', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    class TestResizeObserver {
      observe() {}
      disconnect() {}
    }
    vi.stubGlobal('ResizeObserver', TestResizeObserver);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('hides unknown capability projections', () => {
    act(() => {
      root.render(
        <ReasoningPresetSelector
          projection={{ status: 'unknown', presets: [] }}
          onSelect={vi.fn()}
        />,
      );
    });
    expect(container.querySelector('[data-testid="chat-reasoning-preset-selector-btn"]')).toBeNull();
  });

  it('selects a concrete preset and can return to Auto', async () => {
    const onSelect = vi.fn();
    await act(async () => {
      root.render(
        <ReasoningPresetSelector
          projection={{
            status: 'known',
            default_preset: 'medium',
            presets: [
              { id: 'medium', label: 'Medium', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'medium' }] },
              { id: 'high', label: 'High', order: 20, source: 'models_dev', actions: [{ type: 'effort', value: 'high' }] },
            ],
          }}
          selectedPreset="high"
          onSelect={onSelect}
        />,
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-reasoning-preset-selector-btn"]')?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('[data-preset-id="medium"]')?.click();
    });
    expect(onSelect).toHaveBeenCalledWith('medium');

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-reasoning-preset-selector-btn"]')?.click();
    });
    const auto = Array.from(document.body.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]'))
      .find(item => !item.dataset.presetId);
    await act(async () => {
      auto?.click();
    });
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it('only shows preset sources when visible labels need disambiguation', async () => {
    await act(async () => {
      root.render(
        <ReasoningPresetSelector
          projection={{
            status: 'known',
            presets: [
              { id: 'low', label: 'Low', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'low' }] },
              { id: 'custom-low', label: 'Low', order: 20, source: 'model_config', actions: [{ type: 'effort', value: 'low' }] },
              { id: 'high', label: 'High', order: 30, source: 'adapter_fallback', actions: [{ type: 'effort', value: 'high' }] },
            ],
          }}
          onSelect={vi.fn()}
        />,
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-reasoning-preset-selector-btn"]')?.click();
    });

    expect(document.body.querySelector('[data-preset-id="low"] small')?.textContent)
      .toBe('reasoningSelector.source.models_dev');
    expect(document.body.querySelector('[data-preset-id="custom-low"] small')?.textContent)
      .toBe('reasoningSelector.source.model_config');
    expect(document.body.querySelector('[data-preset-id="high"] small')).toBeNull();
  });
});
