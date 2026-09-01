// @vitest-environment jsdom

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelsDevReasoningCatalog } from '@/infrastructure/api/service-api/AIApi';
import type { ReasoningCatalogProjection, ReasoningConfig } from '../types';
import ReasoningPresetEditor from './ReasoningPresetEditor';

vi.mock('react-i18next', async importOriginal => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => (
      typeof options?.presets === 'string'
        ? `${key}: ${String(options.format)}: ${options.presets}`
        : key
    ),
  }),
}));

interface SelectSpyProps {
  'aria-label'?: string;
  value?: string | number | null;
  options?: Array<{ label: string; value: string | number }>;
  onValueChange?: (value: string | number) => void;
  onCreateValue?: (value: string) => string | number | void;
  disabled?: boolean;
  clearable?: boolean;
}

const selectProps: Record<string, SelectSpyProps> = {};

vi.mock('@bitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  ScrollArea: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  Icon: ({ name, ...props }: { name: string } & React.HTMLAttributes<HTMLSpanElement>) => <span data-icon={name} {...props} />,
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>{children}</button>
  ),
  IconButton: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>{children}</button>
  ),
  Input: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />,
  NumberInput: () => <input type="number" />,
  Switch: () => <input type="checkbox" />,
  Combobox: (props: SelectSpyProps) => {
    const label = props['aria-label'] ?? '';
    selectProps[label] = props;
    return (
      <select
        aria-label={label}
        value={typeof props.value === 'string' ? props.value : ''}
        disabled={props.disabled}
        onChange={(event) => props.onValueChange?.(event.target.value)}
      >
        {props.options?.map(option => (
          <option key={String(option.value)} value={String(option.value)}>{option.label}</option>
        ))}
      </select>
    );
  },
  Select: (props: SelectSpyProps) => {
    const label = props['aria-label'] ?? '';
    selectProps[label] = props;
    return (
      <select
        aria-label={label}
        value={typeof props.value === 'string' ? props.value : ''}
        disabled={props.disabled}
        onChange={(event) => props.onValueChange?.(event.target.value)}
      >
        {props.options?.map(option => (
          <option key={String(option.value)} value={String(option.value)}>{option.label}</option>
        ))}
      </select>
    );
  },
  Textarea: () => <textarea />,
}));

const modelsDevReasoningCatalog: ModelsDevReasoningCatalog = {
  revision: 'test',
  source: 'cache',
  providers: [
    {
      id: 'deepseek',
      name: 'DeepSeek',
      models: [
        {
          id: 'deepseek-v4-flash',
          display_name: 'DeepSeek V4 Flash',
        },
        {
          id: 'deepseek-v4-pro',
        },
      ],
    },
    {
      id: 'github-copilot',
      name: 'GitHub Copilot',
      models: [{ id: 'gpt-5.1-codex', display_name: 'GPT-5.1 Codex' }],
    },
  ],
};

function renderEditor(
  value?: ReasoningConfig,
  onChange?: ReturnType<typeof vi.fn>,
  generatedProjection?: ReasoningCatalogProjection,
  requestFormatLabel?: string,
) {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  act(() => {
    root.render(
      <ReasoningPresetEditor
        value={value ?? { catalog: { source: 'models_dev', provider: '', model: '' }, presets: [] }}
        onChange={onChange ?? vi.fn()}
        generatedProjection={generatedProjection}
        requestFormatLabel={requestFormatLabel}
        modelsDevReasoningCatalog={modelsDevReasoningCatalog}
      />,
    );
  });
  return { container, root };
}

let activeRoot: Root | null = null;
let activeContainer: HTMLDivElement | null = null;

describe('ReasoningPresetEditor models-dev binding', () => {
  beforeEach(() => {
    for (const key of Object.keys(selectProps)) delete selectProps[key];
    activeRoot = null;
    activeContainer = null;
  });

  afterEach(() => {
    if (activeRoot) act(() => activeRoot!.unmount());
    activeContainer?.remove();
  });

  function render(
    value?: ReasoningConfig,
    onChange?: ReturnType<typeof vi.fn>,
    generatedProjection?: ReasoningCatalogProjection,
    requestFormatLabel?: string,
  ) {
    const { container, root } = renderEditor(
      value,
      onChange,
      generatedProjection,
      requestFormatLabel,
    );
    activeRoot = root;
    activeContainer = container;
  }

  it('has an empty provider value when no provider is selected', () => {
    render({ catalog: { source: 'models_dev', provider: '', model: '' }, presets: [] });
    const provider = selectProps['reasoningPresets.catalogProvider'];
    expect(provider).toBeTruthy();
    expect(provider?.options?.map(o => o.value)).toEqual(['', 'deepseek', 'github-copilot']);
    expect(provider?.value).toBe('');
    expect(provider?.clearable).toBe(true);
    expect(provider?.onCreateValue?.('custom')).toBe('custom');
  });

  it('lists only reasoning-capable models of the selected provider', () => {
    render({ catalog: { source: 'models_dev', provider: 'deepseek', model: '' }, presets: [] });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options?.map(o => o.value)).toEqual(['deepseek-v4-flash', 'deepseek-v4-pro']);
    expect(model?.clearable).toBe(true);
    expect(model?.onCreateValue?.('custom')).toBe('custom');
  });

  it('returns empty model options for an unknown provider', () => {
    render({ catalog: { source: 'models_dev', provider: 'unknown', model: '' }, presets: [] });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options ?? []).toHaveLength(0);
  });

  it('lists a provider outside the BitFun built-in overlay', () => {
    render({ catalog: { source: 'models_dev', provider: 'github-copilot', model: '' }, presets: [] });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options?.map(o => o.value)).toEqual(['gpt-5.1-codex']);
  });

  it('warns about explicitly bound models.dev presets unsupported by the API format', () => {
    render(
      {
        catalog: { source: 'models_dev', provider: 'anthropic', model: 'claude-fable-5' },
        presets: [],
      },
      vi.fn(),
      {
        status: 'unknown',
        presets: [],
        unavailable_presets: [
          { id: 'low', label: 'Low', order: 10, source: 'models_dev',
            actions: [{ type: 'effort', value: 'low' }] },
          { id: 'high', label: 'High', order: 20, source: 'models_dev',
            actions: [{ type: 'effort', value: 'high' }] },
        ],
      },
      'OpenAI (chat/completions)',
    );

    const warning = activeContainer?.querySelector('[data-bf-part="unavailableWarning"]');
    expect(warning?.textContent).toContain('reasoningPresets.unavailableTitle');
    expect(warning?.textContent).toContain('OpenAI (chat/completions)');
    expect(warning?.textContent).toContain('Low, High');
    expect(selectProps['reasoningPresets.defaultPreset']?.options?.map(option => option.value))
      .toEqual(['']);
  });

  it('reflects the currently bound provider and model values', () => {
    render({ catalog: { source: 'models_dev', provider: 'github-copilot', model: 'gpt-5.1-codex' }, presets: [] });
    const provider = selectProps['reasoningPresets.catalogProvider'];
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(provider?.value).toBe('github-copilot');
    expect(model?.value).toBe('gpt-5.1-codex');
  });

  it('keeps both bindings clearable and gives custom values an explicit creation callback', () => {
    render();
    const provider = selectProps['reasoningPresets.catalogProvider'];
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(provider?.clearable).toBe(true);
    expect(provider?.onCreateValue?.('provider')).toBe('provider');
    expect(model?.clearable).toBe(true);
    expect(model?.onCreateValue?.('model')).toBe('model');
  });

  it('clears the model when the provider changes', () => {
    const onChange = vi.fn();
    render(
      { catalog: { source: 'models_dev', provider: 'deepseek', model: 'deepseek-v4-flash' }, presets: [] },
      onChange,
    );
    const provider = selectProps['reasoningPresets.catalogProvider'];
    expect(provider).toBeTruthy();
    act(() => {
      provider?.onValueChange?.('github-copilot');
    });
    expect(onChange).toHaveBeenCalledTimes(1);
    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.catalog?.provider).toBe('github-copilot');
    expect(updated.catalog?.model).toBe('');
  });

  it('clearing the provider falls back to the auto catalog source', () => {
    const onChange = vi.fn();
    render(
      { catalog: { source: 'models_dev', provider: 'deepseek', model: 'deepseek-v4-flash' }, presets: [] },
      onChange,
    );
    const provider = selectProps['reasoningPresets.catalogProvider'];
    expect(provider).toBeTruthy();
    act(() => {
      provider?.onValueChange?.('');
    });
    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.catalog).toEqual({ source: 'auto' });
  });

  it('clearing the model falls back to the auto catalog source', () => {
    const onChange = vi.fn();
    render(
      { catalog: { source: 'models_dev', provider: 'github-copilot', model: 'gpt-5.1-codex' }, presets: [] },
      onChange,
    );
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model).toBeTruthy();
    act(() => {
      model?.onValueChange?.('');
    });
    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.catalog).toEqual({ source: 'auto' });
  });

  it('clearing the provider drops a generated default preset', () => {
    const onChange = vi.fn();
    const projection: ReasoningCatalogProjection = {
      status: 'known',
      default_preset: 'high',
      presets: [
        { id: 'high', label: 'High', order: 0, source: 'models_dev',
          actions: [{ type: 'effort', value: 'high' }] },
      ],
    };
    render(
      { catalog: { source: 'models_dev', provider: 'deepseek', model: 'deepseek-v4-flash' },
        default_preset: 'high', presets: [] },
      onChange,
      projection,
    );
    const provider = selectProps['reasoningPresets.catalogProvider'];
    act(() => {
      provider?.onValueChange?.('');
    });
    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.catalog).toEqual({ source: 'auto' });
    expect(updated.default_preset).toBeUndefined();
  });

  it('clearing the model drops a generated default preset', () => {
    const onChange = vi.fn();
    const projection: ReasoningCatalogProjection = {
      status: 'known',
      default_preset: 'high',
      presets: [
        { id: 'high', label: 'High', order: 0, source: 'models_dev',
          actions: [{ type: 'effort', value: 'high' }] },
      ],
    };
    render(
      { catalog: { source: 'models_dev', provider: 'github-copilot', model: 'gpt-5.1-codex' },
        default_preset: 'high', presets: [] },
      onChange,
      projection,
    );
    const model = selectProps['reasoningPresets.catalogModel'];
    act(() => {
      model?.onValueChange?.('');
    });
    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.catalog).toEqual({ source: 'auto' });
    expect(updated.default_preset).toBeUndefined();
  });

  it('re-selecting the current provider keeps the model and default preset', () => {
    const onChange = vi.fn();
    const projection: ReasoningCatalogProjection = {
      status: 'known',
      default_preset: 'high',
      presets: [
        { id: 'high', label: 'High', order: 0, source: 'models_dev',
          actions: [{ type: 'effort', value: 'high' }] },
      ],
    };
    render(
      { catalog: { source: 'models_dev', provider: 'deepseek', model: 'deepseek-v4-flash' },
        default_preset: 'high', presets: [] },
      onChange,
      projection,
    );
    const provider = selectProps['reasoningPresets.catalogProvider'];
    act(() => {
      provider?.onValueChange?.('deepseek');
    });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('re-selecting the current model keeps the default preset', () => {
    const onChange = vi.fn();
    const projection: ReasoningCatalogProjection = {
      status: 'known',
      default_preset: 'high',
      presets: [
        { id: 'high', label: 'High', order: 0, source: 'models_dev',
          actions: [{ type: 'effort', value: 'high' }] },
      ],
    };
    render(
      { catalog: { source: 'models_dev', provider: 'github-copilot', model: 'gpt-5.1-codex' },
        default_preset: 'high', presets: [] },
      onChange,
      projection,
    );
    const model = selectProps['reasoningPresets.catalogModel'];
    act(() => {
      model?.onValueChange?.('gpt-5.1-codex');
    });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('re-selecting the current catalog source keeps the binding', () => {
    const onChange = vi.fn();
    render(
      { catalog: { source: 'models_dev', provider: 'deepseek', model: 'deepseek-v4-flash' },
        default_preset: 'high', presets: [] },
      onChange,
    );
    const source = selectProps['reasoningPresets.catalogSource'];
    expect(source).toBeTruthy();
    act(() => {
      source?.onValueChange?.('models_dev');
    });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('clearing the provider keeps a custom default preset', () => {
    const onChange = vi.fn();
    render(
      { catalog: { source: 'models_dev', provider: 'deepseek', model: 'deepseek-v4-flash' },
        default_preset: 'my-custom',
        presets: [{ id: 'my-custom', label: 'My Custom', actions: [{ type: 'effort', value: 'high' }] }] },
      onChange,
    );
    const provider = selectProps['reasoningPresets.catalogProvider'];
    act(() => {
      provider?.onValueChange?.('');
    });
    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.catalog).toEqual({ source: 'auto' });
    expect(updated.default_preset).toBe('my-custom');
  });
});
