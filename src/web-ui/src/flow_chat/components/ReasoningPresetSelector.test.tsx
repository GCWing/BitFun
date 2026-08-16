/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import zhCnFlowChat from '@/locales/zh-CN/flow-chat.json';
import { ReasoningPresetSelector } from './ReasoningPresetSelector';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string } & Record<string, string>) => {
      const template = ({
        'reasoningSelector.levels.off': 'Off',
        'reasoningSelector.levels.on': 'On',
        'reasoningSelector.levels.low': 'Low',
        'reasoningSelector.levels.medium': 'Medium',
        'reasoningSelector.levels.high': 'High',
        'reasoningSelector.levels.xhigh': 'Extra high',
        'reasoningSelector.levels.max': 'Maximum',
        'reasoningSelector.modes.off': 'Thinking off',
        'reasoningSelector.modes.on': 'Thinking on',
        'reasoningSelector.modes.low': 'Fast mode',
        'reasoningSelector.modes.medium': 'Balanced mode',
        'reasoningSelector.modes.high': 'Deep mode',
        'reasoningSelector.modes.xhigh': 'Intensive mode',
        'reasoningSelector.modes.max': 'Maximum mode',
        'reasoningSelector.modes.custom': 'Model preset',
        'reasoningSelector.current': 'Thinking: {{preset}}',
        'reasoningSelector.currentAuto': 'Thinking: auto ({{preset}})',
      } as Record<string, string>)[key] ?? options?.defaultValue ?? key;
      return template.replace(/\{\{(\w+)\}\}/g, (_, name: string) => String(options?.[name] ?? ''));
    },
  }),
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
    expect(document.body.querySelector('[data-preset-id="high"] small')?.textContent)
      .toBe('Deep mode');
  });

  it('uses the concentric series in the compact trigger and accessible name', async () => {
    const projection = {
      status: 'known' as const,
      default_preset: 'medium',
      presets: [
        { id: 'low', label: 'Low', order: 10, source: 'models_dev' as const, actions: [{ type: 'effort' as const, value: 'low' }] },
        { id: 'medium', label: 'Medium', order: 20, source: 'models_dev' as const, actions: [{ type: 'effort' as const, value: 'medium' }] },
        { id: 'high', label: 'High', order: 30, source: 'models_dev' as const, actions: [{ type: 'effort' as const, value: 'high' }] },
        { id: 'xhigh', label: 'Extra High', order: 40, source: 'models_dev' as const, actions: [{ type: 'effort' as const, value: 'xhigh' }] },
      ],
    };

    await act(async () => {
      root.render(
        <ReasoningPresetSelector
          projection={projection}
          onSelect={vi.fn()}
        />,
      );
    });

    const trigger = container.querySelector<HTMLElement>(
      '[data-testid="chat-reasoning-preset-selector-btn"]',
    );
    // No word beside the mark: the shape is the reading, and the level survives
    // for anyone who cannot see it as the control's own name.
    expect(trigger?.textContent).toBe('');
    expect(trigger?.getAttribute('aria-label')).toBe('Thinking: auto (Medium)');
    const meter = trigger?.querySelector<HTMLElement>(
      '.bitfun-reasoning-preset-selector__status-meter',
    );
    expect(meter?.dataset.intensity).toBe('2');
    expect(meter?.querySelectorAll('.bitfun-reasoning-preset-selector__status-ring'))
      .toHaveLength(2);
    expect(trigger?.querySelectorAll('.bitfun-reasoning-preset-selector__status-meter')).toHaveLength(1);
    expect(trigger?.querySelector('.bitfun-reasoning-preset-selector__label')).toBeNull();

    for (const [presetId, expectedIntensity, expectedRings, hasPeak] of [
      ['low', '1', 1, false],
      ['high', '3', 3, false],
      ['xhigh', '4', 3, true],
    ] as const) {
      await act(async () => {
        root.render(
          <ReasoningPresetSelector
            projection={projection}
            selectedPreset={presetId}
            onSelect={vi.fn()}
          />,
        );
      });
      const updatedMeter = container.querySelector<HTMLElement>(
        '.bitfun-reasoning-preset-selector__status-meter',
      );
      expect(updatedMeter?.dataset.intensity).toBe(expectedIntensity);
      expect(updatedMeter?.querySelectorAll('.bitfun-reasoning-preset-selector__status-ring'))
        .toHaveLength(expectedRings);
      expect(Boolean(updatedMeter?.querySelector('.bitfun-reasoning-preset-selector__status-peak')))
        .toBe(hasPeak);
    }

    expect(
      container
        .querySelector('[data-testid="chat-reasoning-preset-selector-btn"]')
        ?.getAttribute('aria-label'),
    ).toBe('Thinking: Extra high');
  });

  it('renders the common four levels as a low-to-high visual list', async () => {
    await act(async () => {
      root.render(
        <ReasoningPresetSelector
          projection={{
            status: 'known',
            default_preset: 'medium',
            presets: [
              { id: 'xhigh', label: 'Extra High', order: 40, source: 'models_dev', actions: [{ type: 'effort', value: 'xhigh' }] },
              { id: 'low', label: 'Low', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'low' }] },
              { id: 'high', label: 'High', order: 30, source: 'models_dev', actions: [{ type: 'effort', value: 'high' }] },
              { id: 'medium', label: 'Medium', order: 20, source: 'models_dev', actions: [{ type: 'effort', value: 'medium' }] },
            ],
          }}
          selectedPreset="high"
          onSelect={vi.fn()}
        />,
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-reasoning-preset-selector-btn"]')?.click();
    });

    const options = Array.from(
      document.body.querySelectorAll<HTMLButtonElement>(
        '.bitfun-reasoning-preset-selector__options [data-preset-id]',
      ),
    );
    expect(options.map(option => option.dataset.presetId))
      .toEqual(['low', 'medium', 'high', 'xhigh']);
    expect(options.map(option => option.querySelector<HTMLElement>(
      '.bitfun-reasoning-preset-selector__status-meter',
    )?.dataset.intensity)).toEqual(['1', '2', '3', '4']);
    expect(options.map(option => option.querySelector('strong')?.textContent))
      .toEqual(['Low', 'Medium', 'High', 'Extra high']);
    expect(options.map(option => option.querySelector('small')?.textContent))
      .toEqual(['Fast mode', 'Balanced mode', 'Deep mode', 'Intensive mode']);
    expect(options[2]?.querySelector('.bitfun-reasoning-preset-selector__option-check'))
      .not.toBeNull();
    expect(options[3]?.querySelector('.bitfun-reasoning-preset-selector__status-peak'))
      .not.toBeNull();
  });

  it('presents a merged off/on/effort catalog as four user-facing intensity levels', async () => {
    await act(async () => {
      root.render(
        <ReasoningPresetSelector
          projection={{
            status: 'known',
            default_preset: 'low',
            presets: [
              { id: 'max', label: 'Max', order: 40, source: 'models_dev', actions: [{ type: 'effort', value: 'max' }] },
              { id: 'off', label: 'Off', order: 0, source: 'models_dev', actions: [{ type: 'toggle', enabled: false }] },
              { id: 'high', label: 'High', order: 30, source: 'models_dev', actions: [{ type: 'effort', value: 'high' }] },
              { id: 'on', label: 'On', order: 1, source: 'models_dev', actions: [{ type: 'toggle', enabled: true }] },
              { id: 'low', label: 'Low', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'low' }] },
            ],
          }}
          selectedPreset="low"
          onSelect={vi.fn()}
        />,
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>(
        '[data-testid="chat-reasoning-preset-selector-btn"]',
      )?.click();
    });

    const options = Array.from(
      document.body.querySelectorAll<HTMLButtonElement>(
        '.bitfun-reasoning-preset-selector__options [data-preset-id]',
      ),
    );
    expect(options.map(option => option.dataset.presetId))
      .toEqual(['off', 'on', 'low', 'high', 'max']);
    expect(options.map(option => option.querySelector<HTMLElement>(
      '.bitfun-reasoning-preset-selector__status-meter',
    )?.dataset.intensity)).toEqual(['0', '1', '2', '3', '4']);
    expect(options.map(option => option.querySelector('strong')?.textContent))
      .toEqual(['Off', 'Low', 'Medium', 'High', 'Maximum']);
    expect(options.map(option => option.querySelector('small')?.textContent))
      .toEqual(['Thinking off', 'Fast mode', 'Balanced mode', 'Deep mode', 'Maximum mode']);
    expect(options.map(option => option.textContent).join(' '))
      .not.toContain('reasoningSelector.source.models_dev');

    expect(zhCnFlowChat.reasoningSelector.levels)
      .toMatchObject({ off: '关闭', low: '低', medium: '中', high: '高', max: '最高' });
    expect(zhCnFlowChat.reasoningSelector.modes)
      .toMatchObject({
        off: '关闭思考',
        on: '开启思考',
        low: '快速模式',
        high: '深度模式',
        max: '最高强度',
      });
  });

  it('returns focus to the trigger and keeps keyboard motion suppressed while exiting', async () => {
    const onSelect = vi.fn();
    await act(async () => {
      root.render(
        <ReasoningPresetSelector
          projection={{
            status: 'known',
            presets: [
              { id: 'high', label: 'High', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'high' }] },
            ],
          }}
          onSelect={onSelect}
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-reasoning-preset-selector-btn"]',
    );
    trigger?.focus();
    await act(async () => {
      trigger?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      await Promise.resolve();
    });

    const option = document.body.querySelector<HTMLButtonElement>('[data-preset-id="high"]');
    option?.focus();
    await act(async () => {
      option?.click();
    });

    const exitingMenu = document.body.querySelector<HTMLElement>(
      '[data-testid="chat-reasoning-preset-selector-menu"]',
    );
    expect(onSelect).toHaveBeenCalledWith('high');
    expect(document.activeElement).toBe(trigger);
    expect(exitingMenu?.getAttribute('aria-hidden')).toBe('true');
    expect(exitingMenu?.dataset.keyboardOpen).toBe('true');
  });

  it('restores trigger focus when Escape closes a focused menu', async () => {
    await act(async () => {
      root.render(
        <ReasoningPresetSelector
          projection={{
            status: 'known',
            presets: [
              { id: 'medium', label: 'Medium', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'medium' }] },
            ],
          }}
          onSelect={vi.fn()}
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-reasoning-preset-selector-btn"]',
    );
    await act(async () => {
      trigger?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      await Promise.resolve();
    });
    const option = document.body.querySelector<HTMLButtonElement>('[data-preset-id="medium"]');
    option?.focus();

    await act(async () => {
      option?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    });

    expect(document.activeElement).toBe(trigger);
    expect(document.body.querySelector('[data-testid="chat-reasoning-preset-selector-menu"]')
      ?.getAttribute('aria-hidden')).toBe('true');
  });
});
