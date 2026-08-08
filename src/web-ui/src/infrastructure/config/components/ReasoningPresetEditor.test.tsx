// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProviderCatalog } from '@/infrastructure/api/service-api/AIApi';
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

const providerCatalog: ProviderCatalog = {
  revision: 'test',
  source: 'bundle',
  providers: [
    {
      id: 'openbitfun',
      display_order: 0,
      name: 'OpenBitFun',
      description: '',
      requires_api_key: true,
      catalog_provider_ids: ['deepseek', 'zhipuai'],
      catalog_providers: [
        { id: 'deepseek', name: 'DeepSeek' },
        { id: 'zhipuai', name: 'Zhipu AI' },
      ],
      endpoints: [],
      models: [
        {
          id: 'deepseek-v4-flash',
          display_name: 'DeepSeek V4 Flash',
          recommended: true,
          source: 'merged',
          catalog_provider_ids: ['deepseek'],
          capabilities: { chat: true, tool_call: true, reasoning: true, attachment: false, structured_output: true },
        },
        {
          id: 'deepseek-v4-pro',
          recommended: true,
          source: 'merged',
          catalog_provider_ids: ['deepseek'],
          capabilities: { chat: true, tool_call: true, reasoning: true, attachment: false, structured_output: true },
        },
        {
          id: 'glm-5.2',
          recommended: true,
          source: 'merged',
          catalog_provider_ids: ['zhipuai'],
          capabilities: { chat: true, tool_call: true, reasoning: true, attachment: false, structured_output: true },
        },
        {
          id: 'deepseek-v2',
          recommended: false,
          source: 'merged',
          catalog_provider_ids: ['deepseek'],
          capabilities: { chat: true, tool_call: true, reasoning: false, attachment: false, structured_output: false },
        },
      ],
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
        providerCatalog={providerCatalog}
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
          providerCatalog={providerCatalog}
        />,
      );
    });
    const provider = selectProps['reasoningPresets.catalogProvider'];
    expect(provider).toBeTruthy();
    expect(provider?.options?.map(o => o.value)).toEqual(['deepseek', 'zhipuai']);
    expect(provider?.value).toBe('');
  });

  it('lists only reasoning-capable models of the selected provider', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'deepseek', model: '' }, presets: [] }}
          onChange={vi.fn()}
          providerCatalog={providerCatalog}
        />,
      );
    });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options?.map(o => o.value)).toEqual(['deepseek-v4-flash', 'deepseek-v4-pro']);
  });

  it('returns empty model options for an unknown provider', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'unknown', model: '' }, presets: [] }}
          onChange={vi.fn()}
          providerCatalog={providerCatalog}
        />,
      );
    });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options ?? []).toHaveLength(0);
  });

  it('filters models by the selected provider (zhipuai only shows glm-5.2)', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'zhipuai', model: '' }, presets: [] }}
          onChange={vi.fn()}
          providerCatalog={providerCatalog}
        />,
      );
    });
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(model?.options?.map(o => o.value)).toEqual(['glm-5.2']);
  });

  it('reflects the currently bound provider and model values', () => {
    act(() => {
      root.render(
        <ReasoningPresetEditor
          value={{ catalog: { source: 'models_dev', provider: 'zhipuai', model: 'glm-5.2' }, presets: [] }}
          onChange={vi.fn()}
          providerCatalog={providerCatalog}
        />,
      );
    });
    const provider = selectProps['reasoningPresets.catalogProvider'];
    const model = selectProps['reasoningPresets.catalogModel'];
    expect(provider?.value).toBe('zhipuai');
    expect(model?.value).toBe('glm-5.2');
  });
});