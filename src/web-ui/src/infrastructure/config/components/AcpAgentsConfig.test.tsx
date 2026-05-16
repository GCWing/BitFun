// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import AcpAgentsConfig from './AcpAgentsConfig';

const loadJsonConfigMock = vi.hoisted(() => vi.fn());
const getClientsMock = vi.hoisted(() => vi.fn());
const probeClientRequirementsMock = vi.hoisted(() => vi.fn());
const notifyErrorMock = vi.hoisted(() => vi.fn());
const notifySuccessMock = vi.hoisted(() => vi.fn());
const translate = (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => (
  options?.defaultValue ?? _key
);

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: translate,
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    disabled,
    isLoading,
    onClick,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
    isLoading?: boolean;
    onClick?: () => void;
  }) => (
    <button type="button" disabled={disabled || isLoading} onClick={onClick}>
      {children}
    </button>
  ),
  Input: ({
    value,
    onChange,
    placeholder,
  }: {
    value?: string;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
    placeholder?: string;
  }) => <input value={value} onChange={onChange} placeholder={placeholder} />,
  Select: ({
    value,
    onChange,
    options,
  }: {
    value?: string;
    onChange?: (value: string) => void;
    options?: Array<{ value: string; label: string }>;
  }) => (
    <select value={value} onChange={(event) => onChange?.(event.target.value)}>
      {(options ?? []).map((option) => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
  ),
  Textarea: React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
    (props, ref) => <textarea ref={ref} {...props} />,
  ),
}));

vi.mock('./common', () => ({
  ConfigPageContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageHeader: ({ title, subtitle }: { title: string; subtitle: string }) => (
    <header>
      <h1>{title}</h1>
      <p>{subtitle}</p>
    </header>
  ),
  ConfigPageLayout: ({ children }: { children: React.ReactNode }) => <main>{children}</main>,
  ConfigPageSection: ({
    children,
    title,
    description,
  }: {
    children: React.ReactNode;
    title: string;
    description?: string;
  }) => (
    <section>
      <h2>{title}</h2>
      {description ? <p>{description}</p> : null}
      {children}
    </section>
  ),
}));

vi.mock('../../api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {
    loadJsonConfig: loadJsonConfigMock,
    getClients: getClientsMock,
    probeClientRequirements: probeClientRequirementsMock,
    installClientCli: vi.fn(),
    saveJsonConfig: vi.fn(),
  },
}));

vi.mock('../../api/service-api/SystemAPI', () => ({
  systemAPI: {
    openExternal: vi.fn(),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({
    error: notifyErrorMock,
    success: notifySuccessMock,
  }),
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
  }),
}));

describe('AcpAgentsConfig', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    loadJsonConfigMock.mockResolvedValue(JSON.stringify({
      acpClients: {
        opencode: {
          name: 'opencode',
          command: 'opencode',
          args: ['acp'],
          env: {},
          enabled: true,
          readonly: false,
          permissionMode: 'ask',
        },
      },
    }));
    getClientsMock.mockResolvedValue([{
      id: 'opencode',
      name: 'opencode',
      command: 'opencode',
      args: ['acp'],
      enabled: true,
      readonly: false,
      permissionMode: 'ask',
      status: 'configured',
      sessionCount: 0,
      toolName: 'acp__opencode__prompt',
    }]);
    probeClientRequirementsMock.mockResolvedValue([]);

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) {
      act(() => {
        root.unmount();
      });
    }
    container?.remove();
    vi.clearAllMocks();
  });

  it('does not probe external ACP CLIs just by opening the settings tab', async () => {
    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(loadJsonConfigMock).toHaveBeenCalledTimes(1);
    expect(getClientsMock).toHaveBeenCalledTimes(1);
    expect(probeClientRequirementsMock).not.toHaveBeenCalled();
  });
});
