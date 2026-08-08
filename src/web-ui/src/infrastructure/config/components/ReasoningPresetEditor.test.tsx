// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelsDevReasoningCatalog } from '@/infrastructure/api/service-api/AIApi';
import type { ReasoningConfig } from '../types';
import ReasoningPresetEditor from './ReasoningPresetEditor';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

interface SelectSpyProps {
  'aria-label'?: string;
  value?: string | number | (string | number)[] | null;
  options?: Array<{ label: string; value: string | number }>;
  onChange?: (value: string | number | (string | number)[]) => void;
  disabled?: boolean;
  allowCustomValue?: boolean;
  searchable?: boolean;
  clearable?: boolean;
}

const selectProps: Record<string, SelectSpyProps> = {};

vi.mock('@/component-library', () => ({
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
  Select: (props: SelectSpyProps) => {
    const label = props['aria-label'] ?? '';
    selectProps[label] = props;
    return (
      <select
        aria-label={label}
        value={typeof props.value === 'string' ? props.value : ''}
        disabled={props.disabled}
        onChange={(event) => props.onChange?.(event.target.value)}
      >
        {props.options?.map(option => (
          <option key={String(option.value)} value={String(option.value)}>{option.label}</option>
        ))}
      </select>
    );
  },
  Switch: () => <input type="checkbox" />,
  Input: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />,
  NumberInput: () => <input type="number" />,
  Textarea: () => <textarea />,
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
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

function renderEditor(value?: ReasoningConfig) {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  act(() => {
    root.render(
      <ReasoningPresetEditor
        value={value ?? { catalog: { source: 'models_dev', provider: '', model: '' }, presets: [] }}
        onChange={vi.fn()}
        modelsDevReasoningCatalog={modelsDevReasoningCatalog}
      />,
    );
  });
  return { container, root };
}

describe('ReasoningPresetEditor models-dev binding', () => {
  let root: Root;
  let container: HTMLDivElement;

  beforeEach(() => {
    for (const key of Object.keys(selectProps)) delete selectProps[key];
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root?.unmount());
    container?.remove();
  });

  it('has an empty provider value when no provider is selected', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: '', model: '' }, presets: [] }}
          onChange={vi.fn()}
          modelsDevReasoningCatalog={modelsDevReasoningCatalog}
        />,
      );
    });
    const provider = selectProps['reasoningPresets.catalogProvider'];
    expect(provider).toBeTruthy();
    expect(provider?.options?.map(o => o.value)).toEqual(['', 'deepseek', 'github-copilot']);
    expect(provider?.value).toBe('');
    expect(provider?.searchable).toBe(true);
    expect(provider?.clearable).toBe(true);
    expect(provider?.allowCustomValue).toBe(true);
  });

  it('lists only reasoning-capable models of the selected provider', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'deepseek', model: '' }, presets: [] }}
          onChange={vi.fn()}
          modelsDevReasoningCatalog={modelsDevReasoningCatalog}
        />,
      );
    });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options?.map(o => o.value)).toEqual(['deepseek-v4-flash', 'deepseek-v4-pro']);
    expect(model?.searchable).toBe(true);
    expect(model?.clearable).toBe(true);
    expect(model?.allowCustomValue).toBe(true);
  });

  it('returns empty model options for an unknown provider', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'unknown', model: '' }, presets: [] }}
          onChange={vi.fn()}
          modelsDevReasoningCatalog={modelsDevReasoningCatalog}
        />,
      );
    });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options ?? []).toHaveLength(0);
  });

  it('lists a provider outside the BitFun built-in overlay', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'github-copilot', model: '' }, presets: [] }}
          onChange={vi.fn()}
          modelsDevReasoningCatalog={modelsDevReasoningCatalog}
        />,
      );
    });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options?.map(o => o.value)).toEqual(['gpt-5.1-codex']);
  });

  it('reflects the currently bound provider and model values', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'github-copilot', model: 'gpt-5.1-codex' }, presets: [] }}
          onChange={vi.fn()}
          modelsDevReasoningCatalog={modelsDevReasoningCatalog}
        />,
      );
    });
    const provider = selectProps['reasoningPresets.catalogProvider'];
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(provider?.value).toBe('github-copilot');
    expect(model?.value).toBe('gpt-5.1-codex');
  });

  it('clears the model when the provider changes', () => {
    const onChange = vi.fn();
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'deepseek', model: 'deepseek-v4-pro' }, presets: [] }}
          onChange={onChange}
          modelsDevReasoningCatalog={modelsDevReasoningCatalog}
        />,
      );
    });

    act(() => {
      selectProps['reasoningPresets.catalogProvider']?.onChange?.('github-copilot');
    });

    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({
      catalog: { source: 'models_dev', provider: 'github-copilot', model: '' },
    }));
  });
});
