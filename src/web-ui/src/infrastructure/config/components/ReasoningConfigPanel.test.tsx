// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReasoningCatalogProjection, ReasoningConfig } from '../types';
import ReasoningConfigPanel from './ReasoningConfigPanel';

const { projectReasoningCatalog } = vi.hoisted(() => ({
  projectReasoningCatalog: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  aiApi: { projectReasoningCatalog },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
  Button: ({
    children,
    variant: _variant,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: string }) => (
    <button type="button" {...props}>{children}</button>
  ),
}));

vi.mock('./ReasoningPresetEditor', () => ({
  default: ({
    value,
    generatedProjection,
    onChange,
    onValidationChange,
  }: {
    value: ReasoningConfig;
    generatedProjection?: { presets?: Array<{ id: string }> };
    onChange: (value: ReasoningConfig) => void;
    onValidationChange: (invalid: boolean) => void;
  }) => (
    <div>
      <button
        type="button"
        data-testid="change-reasoning"
        onClick={() => onChange({
          ...value,
          presets: [{ id: 'custom', actions: [{ type: 'effort', value: 'high' }] }],
        })}
      >
        change
      </button>
      <button
        type="button"
        data-testid="bind-models-dev"
        onClick={() => onChange({
          ...value,
          catalog: { source: 'models_dev', provider: 'openai', model: 'gpt-test' },
        })}
      >
        bind
      </button>
      <span data-testid="generated-presets">
        {generatedProjection?.presets?.map(preset => preset.id).join(',') ?? ''}
      </span>
      <button
        type="button"
        data-testid="invalidate-reasoning"
        onClick={() => onValidationChange(true)}
      >
        invalidate
      </button>
    </div>
  ),
}));

describe('ReasoningConfigPanel', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    projectReasoningCatalog.mockReset();
    container = document.createElement('div');
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  it('keeps edits local until Apply is selected', () => {
    const onApply = vi.fn();
    act(() => root.render(
      <ReasoningConfigPanel
        value={{ catalog: { source: 'auto' }, presets: [] }}
        onCancel={vi.fn()}
        onApply={onApply}
      />,
    ));

    act(() => container.querySelector<HTMLButtonElement>('[data-testid="change-reasoning"]')?.click());
    expect(onApply).not.toHaveBeenCalled();

    const apply = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent === 'reasoningPresets.apply');
    act(() => apply?.click());

    expect(onApply).toHaveBeenCalledWith({
      reasoning: {
        catalog: { source: 'auto' },
        presets: [{ id: 'custom', actions: [{ type: 'effort', value: 'high' }] }],
      },
      projectionCatalog: { source: 'auto' },
      projection: undefined,
    });
  });

  it('publishes nested edits to the provider draft without treating them as persisted', () => {
    const onDraftChange = vi.fn();
    const onApply = vi.fn();
    act(() => root.render(
      <ReasoningConfigPanel
        value={{ catalog: { source: 'auto' }, presets: [] }}
        onCancel={vi.fn()}
        onApply={onApply}
        onDraftChange={onDraftChange}
      />,
    ));
    onDraftChange.mockClear();

    act(() => container.querySelector<HTMLButtonElement>('[data-testid="change-reasoning"]')?.click());

    expect(onDraftChange).toHaveBeenLastCalledWith({
      reasoning: {
        catalog: { source: 'auto' },
        presets: [{ id: 'custom', actions: [{ type: 'effort', value: 'high' }] }],
      },
      projectionCatalog: { source: 'auto' },
      projection: undefined,
    });
    expect(onApply).not.toHaveBeenCalled();
  });

  it('cancels without applying and blocks invalid drafts', () => {
    const onCancel = vi.fn();
    const onApply = vi.fn();
    act(() => root.render(
      <ReasoningConfigPanel
        value={{ catalog: { source: 'auto' }, presets: [] }}
        onCancel={onCancel}
        onApply={onApply}
      />,
    ));

    act(() => container.querySelector<HTMLButtonElement>('[data-testid="invalidate-reasoning"]')?.click());
    const buttons = Array.from(container.querySelectorAll('button'));
    const apply = buttons.find(button => button.textContent === 'reasoningPresets.apply');
    const cancel = buttons.find(button => button.textContent === 'actions.cancel');
    expect(apply?.disabled).toBe(true);

    act(() => cancel?.click());
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onApply).not.toHaveBeenCalled();
  });

  it('refreshes generated presets after an explicit models.dev binding change', async () => {
    const onApply = vi.fn();
    const modelsDevProjection: ReasoningCatalogProjection = {
      status: 'known',
      presets: [
        { id: 'low', label: 'Low', order: 0, source: 'models_dev', actions: [] },
        { id: 'high', label: 'High', order: 1, source: 'models_dev', actions: [] },
      ],
    };
    projectReasoningCatalog
      .mockResolvedValueOnce({ status: 'unknown', presets: [] })
      .mockResolvedValueOnce(modelsDevProjection);

    await act(async () => root.render(
      <ReasoningConfigPanel
        value={{ catalog: { source: 'auto' }, presets: [] }}
        projectionRequest={{
          provider: 'responses',
          modelName: 'gateway-alias',
          baseUrl: 'https://gateway.example.com/v1',
        }}
        onCancel={vi.fn()}
        onApply={onApply}
      />,
    ));

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="bind-models-dev"]')?.click();
      await Promise.resolve();
    });

    expect(projectReasoningCatalog).toHaveBeenLastCalledWith({
      provider: 'responses',
      modelName: 'gateway-alias',
      baseUrl: 'https://gateway.example.com/v1',
      reasoning: {
        catalog: { source: 'models_dev', provider: 'openai', model: 'gpt-test' },
        presets: [],
      },
    });
    expect(container.querySelector('[data-testid="generated-presets"]')?.textContent)
      .toBe('low,high');

    const apply = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent === 'reasoningPresets.apply');
    act(() => apply?.click());

    expect(onApply).toHaveBeenCalledWith({
      reasoning: {
        catalog: { source: 'models_dev', provider: 'openai', model: 'gpt-test' },
        presets: [],
      },
      projectionCatalog: { source: 'models_dev', provider: 'openai', model: 'gpt-test' },
      projection: modelsDevProjection,
    });
  });
});
