// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import BasicsConfig from './BasicsConfig';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const getConfigMock = vi.hoisted(() => vi.fn());
const setConfigMock = vi.hoisted(() => vi.fn());
const clearCacheMock = vi.hoisted(() => vi.fn());
const getRuntimeLoggingInfoMock = vi.hoisted(() => vi.fn());
const getLaunchAtLoginMock = vi.hoisted(() => vi.fn());
const getPreventSleepMock = vi.hoisted(() => vi.fn());
const translateMock = vi.hoisted(() => vi.fn((key: string) => key));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: translateMock }),
}));

vi.mock('../services/ConfigManager', () => ({
  configManager: {
    getConfig: getConfigMock,
    setConfig: setConfigMock,
    clearCache: clearCacheMock,
  },
}));

vi.mock('@/shared/utils/logger', () => {
  const fn = () => vi.fn();
  const contextLogger = { trace: fn(), debug: fn(), info: fn(), warn: fn(), error: fn() };
  return {
    createLogger: () => contextLogger,
    logger: contextLogger,
    log: contextLogger,
  };
});

vi.mock('@/infrastructure/api', () => ({
  configAPI: {
    getRuntimeLoggingInfo: getRuntimeLoggingInfoMock,
    exportDiagnosticsBundle: vi.fn(),
    setConfig: vi.fn(),
  },
  workspaceAPI: {
    revealInExplorer: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('@/infrastructure/api/service-api/SystemAPI', () => ({
  systemAPI: {
    getLaunchAtLoginEnabled: getLaunchAtLoginMock,
    setLaunchAtLoginEnabled: vi.fn(),
    getPreventSleepEnabled: getPreventSleepMock,
    setPreventSleepEnabled: vi.fn(),
  },
}));

vi.mock('@/tools/terminal/services', () => ({
  getTerminalService: () => ({ getAvailableShells: vi.fn().mockResolvedValue([]) }),
  refreshTerminalPanelPosition: vi.fn(),
  setTerminalPanelPosition: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/component-library', () => ({
  Alert: ({ message, description }: { message?: string; description?: string }) => (
    <div role="alert">
      {message}
      {description}
    </div>
  ),
  Button: ({
    children,
    disabled,
    onClick,
    'data-testid': testId,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
    onClick?: () => void;
    'data-testid'?: string;
  }) => (
    <button type="button" disabled={disabled} onClick={onClick} data-testid={testId}>
      {children}
    </button>
  ),
  Input: ({
    value,
    onChange,
    disabled,
    'aria-label': ariaLabel,
  }: {
    value: string;
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
    disabled?: boolean;
    'aria-label'?: string;
  }) => (
    <input
      type="text"
      value={value}
      disabled={disabled}
      aria-label={ariaLabel}
      onChange={onChange}
    />
  ),
  NumberInput: () => <input type="number" />,
  Select: () => <select />,
  Switch: () => <input type="checkbox" />,
  Tooltip: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageLoading: ({ text }: { text: string }) => <div>{text}</div>,
  ConfigPageMessage: () => null,
}));

vi.mock('./common', () => ({
  ConfigPageLayout: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageSection: ({ title, children }: { title: string; children: React.ReactNode }) => (
    <section>
      <h3>{title}</h3>
      {children}
    </section>
  ),
  ConfigPageRow: ({ label, children }: { label: React.ReactNode; children: React.ReactNode }) => (
    <div>
      <span>{label}</span>
      {children}
    </div>
  ),
  ConfigPageHeader: ({ title, subtitle }: { title: string; subtitle?: string }) => (
    <header>
      <h2>{title}</h2>
      <p>{subtitle}</p>
    </header>
  ),
}));

let container: HTMLElement;
let root: Root;

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  vi.clearAllMocks();
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

async function renderBasics(): Promise<void> {
  await act(async () => {
    root.render(<BasicsConfig />);
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('BasicsConfig knowledge base root (UX-P1-3)', () => {
  it('loads and renders the configured ai.knowledge_base_root', async () => {
    getConfigMock.mockImplementation((key: string) => {
      if (key === 'ai.knowledge_base_root') return Promise.resolve('C:/kb/root');
      if (key === 'app.logging.level') return Promise.resolve('info');
      if (key === 'app.logging.include_sensitive_diagnostics') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    getRuntimeLoggingInfoMock.mockResolvedValue({
      sessionLogDir: '/tmp/logs',
      effectiveLevel: 'info',
      previousUnexpectedExit: null,
    });
    getLaunchAtLoginMock.mockResolvedValue(false);
    getPreventSleepMock.mockResolvedValue(false);

    await renderBasics();

    const input = container.querySelector<HTMLInputElement>('[aria-label="knowledgeBase.rootLabel"]');
    expect(input).not.toBeNull();
    expect(input!.value).toBe('C:/kb/root');
  });

  it('persists a typed knowledge base root', async () => {
    getConfigMock.mockImplementation((key: string) => {
      if (key === 'ai.knowledge_base_root') return Promise.resolve('');
      return Promise.resolve(null);
    });
    getRuntimeLoggingInfoMock.mockResolvedValue({
      sessionLogDir: '/tmp/logs',
      effectiveLevel: 'info',
      previousUnexpectedExit: null,
    });
    getLaunchAtLoginMock.mockResolvedValue(false);
    getPreventSleepMock.mockResolvedValue(false);
    setConfigMock.mockResolvedValue(undefined);

    await renderBasics();

    const input = container.querySelector<HTMLInputElement>('[aria-label="knowledgeBase.rootLabel"]');
    expect(input).not.toBeNull();
    await act(async () => {
      // React 受控组件需要原生 value setter + input 事件才会更新 state。
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value'
      )?.set;
      setter?.call(input, 'D:/docs/kb');
      input!.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });
    await act(async () => {
      const saveButton = container.querySelector<HTMLButtonElement>(
        '[data-testid="basics-knowledge-base-save"]'
      );
      expect(saveButton).not.toBeNull();
      saveButton!.click();
      await Promise.resolve();
    });

    expect(setConfigMock).toHaveBeenCalledWith('ai.knowledge_base_root', 'D:/docs/kb');
    expect(clearCacheMock).toHaveBeenCalled();
  });

  it('clears the root when the input is emptied', async () => {
    getConfigMock.mockImplementation((key: string) => {
      if (key === 'ai.knowledge_base_root') return Promise.resolve('C:/kb/root');
      return Promise.resolve(null);
    });
    getRuntimeLoggingInfoMock.mockResolvedValue({
      sessionLogDir: '/tmp/logs',
      effectiveLevel: 'info',
      previousUnexpectedExit: null,
    });
    getLaunchAtLoginMock.mockResolvedValue(false);
    getPreventSleepMock.mockResolvedValue(false);

    await renderBasics();

    const input = container.querySelector<HTMLInputElement>('[aria-label="knowledgeBase.rootLabel"]');
    expect(input).not.toBeNull();
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value'
      )?.set;
      setter?.call(input, '');
      input!.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });
    await act(async () => {
      const saveButton = container.querySelector<HTMLButtonElement>(
        '[data-testid="basics-knowledge-base-save"]'
      );
      expect(saveButton).not.toBeNull();
      saveButton!.click();
      await Promise.resolve();
    });

    expect(setConfigMock).toHaveBeenCalledWith('ai.knowledge_base_root', '');
  });
});
