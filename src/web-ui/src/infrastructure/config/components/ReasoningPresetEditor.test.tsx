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
    t: (key: string, options?: Record<string, unknown>) => {
      if (typeof options?.format !== 'string') return key;
      return typeof options.presets === 'string'
        ? `${key}: ${options.format}: ${options.presets}`
        : `${key}: ${options.format}`;
    },
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

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
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
  Switch: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input type="checkbox" {...props} />,
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
  Textarea: ({
    invalid: _invalid,
    errorMessage: _errorMessage,
    ...props
  }: React.TextareaHTMLAttributes<HTMLTextAreaElement> & {
    invalid?: boolean;
    errorMessage?: React.ReactNode;
  }) => <textarea {...props} />,
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

describe('ReasoningPresetEditor', () => {
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

  it('lists a provider outside the OpenBitFun built-in overlay', () => {
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

    const warning = activeContainer?.querySelector('[data-openbitfun-part="unavailableWarning"]');
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

  it('creates new custom presets with exactly one request body patch', () => {
    const onChange = vi.fn();
    render({ catalog: { source: 'auto' }, presets: [] }, onChange);

    const addPreset = Array.from(activeContainer?.querySelectorAll('button') ?? [])
      .find(button => button.textContent === 'reasoningPresets.add');
    act(() => addPreset?.click());

    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.presets).toHaveLength(1);
    expect(updated.presets[0]?.actions).toEqual([
      { type: 'request_patch', body: {} },
    ]);
    expect(activeContainer?.textContent).not.toContain('reasoningPresets.addAction');
  });

  it('edits a single request body patch without exposing action controls', () => {
    const onChange = vi.fn();
    render({
      catalog: { source: 'auto' },
      presets: [{
        id: 'custom',
        label: 'Custom',
        actions: [{ type: 'request_patch', body: { reasoning: { effort: 'low' } } }],
      }],
    }, onChange, undefined, 'OpenAI (responses)');

    act(() => activeContainer
      ?.querySelector<HTMLButtonElement>('button[aria-label="Custom"]')
      ?.click());
    const textarea = activeContainer?.querySelector<HTMLTextAreaElement>(
      'textarea[aria-label="reasoningPresets.settingPatch"]',
    );
    expect(textarea?.value).toContain('"effort": "low"');
    expect(activeContainer?.querySelector('[data-openbitfun-part="actionControls"]')).toBeNull();
    expect(textarea?.placeholder).toContain('OpenAI (responses)');

    act(() => {
      const valueSetter = Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        'value',
      )?.set;
      valueSetter?.call(textarea, '{"reasoning":{"effort":"high"}}');
      textarea?.dispatchEvent(new Event('input', { bubbles: true }));
    });

    const updated = onChange.mock.calls.at(-1)?.[0] as ReasoningConfig;
    expect(updated.presets[0]?.actions).toEqual([{
      type: 'request_patch',
      body: { reasoning: { effort: 'high' } },
    }]);
  });

  it('shows the common request patch example as a placeholder for an empty patch', () => {
    render({
      catalog: { source: 'auto' },
      presets: [{
        id: 'custom',
        label: 'Custom',
        actions: [{ type: 'request_patch', body: {} }],
      }],
    }, vi.fn(), undefined, 'Gemini');

    act(() => activeContainer
      ?.querySelector<HTMLButtonElement>('button[aria-label="Custom"]')
      ?.click());
    const textarea = activeContainer?.querySelector<HTMLTextAreaElement>(
      'textarea[aria-label="reasoningPresets.settingPatch"]',
    );

    expect(textarea?.value).toBe('');
    expect(textarea?.placeholder).toBe('reasoningPresets.patchPlaceholder: Gemini');
    expect(activeContainer?.querySelector('.openbitfun-reasoning-preset-editor__row-preview')).toBeNull();
  });

  it('preserves legacy actions until the user explicitly replaces them', () => {
    const onChange = vi.fn();
    render({
      catalog: { source: 'auto' },
      presets: [{
        id: 'legacy',
        label: 'Legacy',
        actions: [
          { type: 'effort', value: 'high' },
          { type: 'request_patch', body: { reasoning: { summary: 'auto' } } },
          { type: 'request_patch', body: { include: ['reasoning.encrypted_content'] } },
        ],
      }],
    }, onChange);

    act(() => activeContainer
      ?.querySelector<HTMLButtonElement>('button[aria-label="Legacy"]')
      ?.click());
    const legacyActions = activeContainer?.querySelectorAll('[data-openbitfun-part="action"]');
    expect(legacyActions).toHaveLength(3);
    expect(activeContainer?.textContent).toContain('reasoningPresets.legacyTitle');
    expect(activeContainer?.textContent).toContain('high');
    expect(activeContainer?.textContent).toContain('reasoning.encrypted_content');
    expect(onChange).not.toHaveBeenCalled();

    const convert = Array.from(activeContainer?.querySelectorAll('button') ?? [])
      .find(button => button.textContent === 'reasoningPresets.convertToSinglePatch');
    act(() => convert?.click());

    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.presets[0]?.actions).toEqual([
      { type: 'request_patch', body: {} },
    ]);
  });

  it('adds a request body patch when an actionless preset is enabled', () => {
    const onChange = vi.fn();
    render({
      catalog: { source: 'auto' },
      presets: [{ id: 'disabled', label: 'Disabled', disabled: true, actions: [] }],
    }, onChange);

    act(() => activeContainer
      ?.querySelector<HTMLInputElement>('input[aria-label="reasoningPresets.enabled"]')
      ?.click());

    const updated = onChange.mock.calls[0][0] as ReasoningConfig;
    expect(updated.presets[0]?.disabled).toBe(false);
    expect(updated.presets[0]?.actions).toEqual([
      { type: 'request_patch', body: {} },
    ]);
  });
});
