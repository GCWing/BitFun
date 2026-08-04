import { BASE_DISPATCH_CAPABILITIES } from './dispatchPreflight';
// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DispatchInstallDialog } from './DispatchInstallDialog';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  probeTarget: vi.fn(),
  syncModelConfig: vi.fn(),
  confirmWarning: vi.fn(),
  getConfig: vi.fn(),
  getFreshConfig: vi.fn(),
  resolveRevision: vi.fn(),
  modalOnClose: null as (() => void) | null,
  modalLifecycleProps: null as {
    closeOnOverlayClick?: boolean;
    showCloseButton?: boolean;
  } | null,
}));

vi.mock('./dispatchApi', () => ({
  dispatchApi: {
    probeTarget: mocks.probeTarget,
    syncModelConfig: mocks.syncModelConfig,
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/infrastructure/config', () => ({
  configManager: { getConfig: mocks.getConfig },
}));

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: { getConfig: mocks.getFreshConfig },
}));

vi.mock('@/infrastructure/api/service-api/GitAPI', () => ({
  gitAPI: { resolveRevision: mocks.resolveRevision },
}));

vi.mock('@/infrastructure/config/services/modelConfigs', () => ({
  getModelDisplayName: (config: { name?: string; model_name?: string }) =>
    `${config.name ?? ''}/${config.model_name ?? ''}`,
}));

vi.mock('@/component-library', () => ({
  Alert: ({ message }: { message: string }) => <div role="alert">{message}</div>,
  Button: ({
    children,
    disabled,
    onClick,
  }: React.PropsWithChildren<{
    disabled?: boolean;
    onClick?: React.MouseEventHandler<HTMLButtonElement>;
  }>) => (
    <button type="button" disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
  Input: ({
    disabled,
    onChange,
    onKeyDown,
    placeholder,
    value,
  }: {
    disabled?: boolean;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
    onKeyDown?: React.KeyboardEventHandler<HTMLInputElement>;
    placeholder?: string;
    value?: string;
  }) => (
    <input
      disabled={disabled}
      onChange={onChange}
      onKeyDown={onKeyDown}
      placeholder={placeholder}
      value={value}
    />
  ),
  Modal: ({
    children,
    closeOnOverlayClick,
    isOpen,
    onClose,
    showCloseButton,
  }: React.PropsWithChildren<{
    closeOnOverlayClick?: boolean;
    isOpen: boolean;
    onClose: () => void;
    showCloseButton?: boolean;
  }>) => {
    mocks.modalOnClose = onClose;
    mocks.modalLifecycleProps = {
      closeOnOverlayClick,
      showCloseButton,
    };
    return isOpen ? <div>{children}</div> : null;
  },
  confirmWarning: mocks.confirmWarning,
}));

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, reject, resolve };
}

describe('DispatchInstallDialog target preparation', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.modalOnClose = null;
    mocks.modalLifecycleProps = null;
    mocks.probeTarget.mockResolvedValue({
      cliInstalled: false,
      os: 'linux',
      arch: 'x86_64',
      installSupported: true,
      release: {
        version: '1.2.3',
        target: 'x86_64-unknown-linux-gnu',
        url: 'https://example.test/bitfun',
        sha256: 'abc123',
      },
    });
    mocks.confirmWarning.mockResolvedValue(true);
    mocks.getConfig.mockResolvedValue([]);
    mocks.getFreshConfig.mockResolvedValue(undefined);
    mocks.resolveRevision.mockResolvedValue('a'.repeat(40));
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('shows the verified release as automatic and follows the worktree copy setting', async () => {
    const onReady = vi.fn();
    mocks.getFreshConfig.mockResolvedValue({ copyLocalChanges: true });

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{
            kind: 'ssh',
            connectionId: 'ssh-1',
            displayName: 'build-host',
          }}
          sourceWorkspacePath="/home/me/project"
          onClose={vi.fn()}
          onReady={onReady}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.installAutomaticDescription');
    expect(container.textContent).toContain('1.2.3');
    expect(container.textContent).toContain('abc123');
    expect(container.querySelector('details')?.open).toBe(false);
    expect(container.textContent).not.toContain('dispatch.installConfirm');
    expect(mocks.modalLifecycleProps).toEqual({
      closeOnOverlayClick: true,
      showCloseButton: true,
    });
    const includeUncommitted = container.querySelector<HTMLInputElement>('input[type="checkbox"]');
    expect(includeUncommitted?.checked).toBe(true);

    await act(async () => {
      Array.from(container.querySelectorAll('button'))
        .find(button => button.textContent?.includes('dispatch.approvalReject'))
        ?.click();
    });

    await act(async () => {
      Array.from(container.querySelectorAll('button'))
        .find(button => button.textContent?.includes('dispatch.useTarget'))
        ?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.getFreshConfig).toHaveBeenCalledWith('app.worktrees', {
      skipRetryOnNotFound: true,
    });
    expect(mocks.resolveRevision).toHaveBeenCalledWith('/home/me/project', 'HEAD');
    expect(onReady).toHaveBeenCalledWith(expect.objectContaining({
      baseRef: 'HEAD',
      includeUncommitted: true,
      approvalPolicy: 'reject-and-report',
      request: { kind: 'ssh', connectionId: 'ssh-1', workspacePath: '' },
    }));
  });

  it('does not overwrite a user choice when the worktree default resolves late', async () => {
    const worktreeSettings = createDeferred<{ copyLocalChanges: boolean }>();
    mocks.getFreshConfig.mockReturnValue(worktreeSettings.promise);

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{ kind: 'ssh', connectionId: 'ssh-1', displayName: 'build-host' }}
          sourceWorkspacePath="/home/me/project"
          onClose={vi.fn()}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
    });

    const includeUncommitted = container.querySelector<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    expect(includeUncommitted?.checked).toBe(false);

    await act(async () => {
      includeUncommitted?.click();
    });
    expect(includeUncommitted?.checked).toBe(true);

    await act(async () => {
      worktreeSettings.resolve({ copyLocalChanges: false });
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(includeUncommitted?.checked).toBe(true);
  });

  it('keeps setup open and reports an invalid base revision before creating a session', async () => {
    const onReady = vi.fn();
    mocks.resolveRevision.mockRejectedValue(new Error('unknown revision'));

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{ kind: 'ssh', connectionId: 'ssh-1', displayName: 'build-host' }}
          sourceWorkspacePath="/home/me/project"
          onClose={vi.fn()}
          onReady={onReady}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    const baseRefInput = container.querySelector<HTMLInputElement>(
      '.dispatch-install-dialog__base-ref input',
    );
    await act(async () => {
      if (baseRefInput) {
        Object.getOwnPropertyDescriptor(
          HTMLInputElement.prototype,
          'value',
        )?.set?.call(baseRefInput, 'missing/ref');
        baseRefInput.dispatchEvent(new Event('input', { bubbles: true }));
      }
      Array.from(container.querySelectorAll('button'))
        .find(button => button.textContent?.includes('dispatch.approvalReject'))
        ?.click();
    });

    await act(async () => {
      Array.from(container.querySelectorAll('button'))
        .find(button => button.textContent?.includes('dispatch.useTarget'))
        ?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.resolveRevision).toHaveBeenCalledWith(
      '/home/me/project',
      'missing/ref',
    );
    expect(onReady).not.toHaveBeenCalled();
    expect(container.textContent).toContain('dispatch.baseRefInvalid');
    expect(container.querySelector('.dispatch-install-dialog')).not.toBeNull();
  });

  it('never offers to compile on the target and explains why it cannot be prepared', async () => {
    // A target no published binary fits. Preparing it is not something this
    // controller can do, so the dialog says so instead of offering to build
    // BitFun on someone else's machine.
    mocks.probeTarget.mockResolvedValue({
      cliInstalled: false,
      os: 'linux',
      arch: 'x86_64',
      installSupported: false,
      prebuiltIncompatible: 'target uses musl libc',
    });

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{ kind: 'ssh', connectionId: 'ssh-1', displayName: 'alpine-host' }}
          sourceWorkspacePath="/home/me/project"
          onClose={vi.fn()}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.installUnavailable');
    expect(container.textContent).not.toContain('target uses musl libc');
    expect(container.textContent).not.toContain('sourceBuild');
    expect(container.textContent).not.toContain('dispatch.installAutomaticDescription');

    await act(async () => {
      Array.from(container.querySelectorAll('button'))
        .find(button => button.textContent?.includes('dispatch.approvalReject'))
        ?.click();
    });
    const useTarget = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.useTarget'));
    expect(
      useTarget?.disabled,
      'a target that cannot be prepared must not be selectable',
    ).toBe(true);
  });

  it('keeps protocol capability names and probe failures out of the user interface', async () => {
    mocks.probeTarget.mockResolvedValueOnce({
      cliInstalled: true,
      os: 'linux',
      arch: 'x86_64',
      installSupported: false,
      protocol: {
        protocolVersion: 4,
        cliVersion: '1.2.3',
        os: 'linux',
        arch: 'x86_64',
        capabilities: BASE_DISPATCH_CAPABILITIES.filter(
          capability => capability !== 'workspace_git_sync',
        ),
        modelConfigured: true,
        availableModels: ['model-a'],
      },
    });

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{ kind: 'ssh', connectionId: 'ssh-1', displayName: 'build-host' }}
          sourceWorkspacePath="/home/me/project"
          onClose={vi.fn()}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.cliUpdateRequired');
    expect(container.textContent).not.toContain('workspace_git_sync');

    mocks.probeTarget.mockRejectedValueOnce(
      new Error('ssh handshake failed at internal transport stage'),
    );
    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{ kind: 'ssh', connectionId: 'ssh-2', displayName: 'backup-host' }}
          sourceWorkspacePath="/home/me/project"
          onClose={vi.fn()}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.probeFailed');
    expect(container.textContent).not.toContain('internal transport stage');
  });

  it('explains the Git baseline and never offers a snapshot delivery mode', async () => {
    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{ kind: 'ssh', connectionId: 'ssh-1', displayName: 'build-host' }}
          sourceWorkspacePath="/home/me/project"
          onClose={vi.fn()}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.baselineSource');
    expect(container.textContent).toContain('dispatch.baselineDescription');
    expect(container.textContent).toContain('dispatch.baseRefHint');
    expect(container.textContent).toContain('dispatch.includeUncommittedHint');
    expect(container.textContent).not.toContain('dispatch.deliverySnapshot');
    expect(container.textContent).not.toContain('dispatch.snapshotResultLocationHint');
  });

  it('preserves protocol v4 target model facts without a delivery-mode choice', async () => {
    const onReady = vi.fn();
    mocks.probeTarget.mockResolvedValue({
      cliInstalled: true,
      os: 'linux',
      arch: 'x86_64',
      installSupported: false,
      protocol: {
        protocolVersion: 4,
        cliVersion: '1.2.3',
        os: 'linux',
        arch: 'x86_64',
        capabilities: [...BASE_DISPATCH_CAPABILITIES, 'approval_remote'],
        modelConfigured: true,
        availableModels: ['model-a', 'model-b'],
        defaultModel: 'model-b',
      },
    });

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{ kind: 'ssh', connectionId: 'ssh-1', displayName: 'build-host' }}
          sourceWorkspacePath="/home/me/project"
          onClose={vi.fn()}
          onReady={onReady}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    const remoteApproval = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.approvalRemote'));
    await act(async () => {
      remoteApproval?.click();
    });
    const useTarget = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.useTarget'));
    expect(useTarget?.disabled).toBe(false);

    await act(async () => {
      useTarget?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(onReady).toHaveBeenCalledWith(expect.objectContaining({
      includeUncommitted: false,
      approvalPolicy: 'remote',
      availableModels: ['model-a', 'model-b'],
      defaultModel: 'model-b',
    }));
  });

});

describe('DispatchInstallDialog model configuration sync', () => {
  let container: HTMLDivElement;
  let root: Root;
  let modelConfigured: boolean;

  const target = {
    kind: 'ssh' as const,
    connectionId: 'ssh-1',
    displayName: 'build-host',
  };

  function probeResult() {
    return {
      cliInstalled: true,
      os: 'linux',
      arch: 'x86_64',
      installSupported: true,
      protocol: {
        protocolVersion: 4,
        cliVersion: '1.2.3',
        os: 'linux',
        arch: 'x86_64',
        capabilities: [...BASE_DISPATCH_CAPABILITIES],
        modelConfigured,
        availableModels: modelConfigured ? ['claude'] : [],
        defaultModel: modelConfigured ? 'claude' : undefined,
      },
    };
  }

  function syncButton() {
    return Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.syncModelConfirm'));
  }

  async function mount(onClose = vi.fn()) {
    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={target}
          onClose={onClose}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  beforeEach(() => {
    vi.clearAllMocks();
    modelConfigured = false;
    mocks.modalOnClose = null;
    mocks.probeTarget.mockImplementation(async () => probeResult());
    mocks.confirmWarning.mockResolvedValue(true);
    mocks.getConfig.mockResolvedValue([
      { id: 'claude', enabled: true, api_key: 'secret' },
    ]);
    mocks.getFreshConfig.mockResolvedValue(undefined);
    mocks.resolveRevision.mockResolvedValue('a'.repeat(40));
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('hides model sync after the target matches this device', async () => {
    await mount();
    expect(syncButton()).toBeDefined();

    mocks.syncModelConfig.mockImplementation(async () => {
      modelConfigured = true;
    });
    const probesBeforeSync = mocks.probeTarget.mock.calls.length;

    await act(async () => {
      syncButton()?.click();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.confirmWarning).toHaveBeenCalledTimes(1);
    expect(mocks.syncModelConfig).toHaveBeenCalledWith('ssh-1');
    // The sync re-probes so the model check reflects the target, not the write.
    expect(mocks.probeTarget.mock.calls.length).toBeGreaterThan(probesBeforeSync);
    expect(syncButton()).toBeUndefined();
  });

  it('does not write the credential-bearing config when the confirmation is declined', async () => {
    await mount();
    mocks.confirmWarning.mockResolvedValue(false);

    await act(async () => {
      syncButton()?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.syncModelConfig).not.toHaveBeenCalled();
    expect(syncButton()).toBeDefined();
  });

  it('keeps the dialog open while model sync is in progress', async () => {
    const sync = createDeferred<void>();
    const onClose = vi.fn();
    mocks.syncModelConfig.mockReturnValue(sync.promise);
    await mount(onClose);

    await act(async () => {
      syncButton()?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.syncModelConfig).toHaveBeenCalledTimes(1);
    const probesBeforeSettle = mocks.probeTarget.mock.calls.length;

    await act(async () => {
      mocks.modalOnClose?.();
      await Promise.resolve();
    });

    expect(onClose).not.toHaveBeenCalled();
    expect(mocks.modalLifecycleProps).toEqual({
      closeOnOverlayClick: false,
      showCloseButton: false,
    });

    await act(async () => {
      modelConfigured = true;
      sync.resolve(undefined);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.probeTarget.mock.calls.length).toBeGreaterThan(probesBeforeSettle);
    expect(syncButton()).toBeUndefined();
  });
});

describe('DispatchInstallDialog target model readout', () => {
  let container: HTMLDivElement;
  let root: Root;

  const target = {
    kind: 'ssh' as const,
    connectionId: 'ssh-1',
    displayName: 'build-host',
  };

  function localModel(id: string, modelName: string) {
    return {
      id,
      name: 'Anthropic',
      model_name: modelName,
      provider: 'anthropic',
      base_url: 'https://example.test',
      api_key: 'secret',
      enabled: true,
      category: 'chat',
      capabilities: [],
    };
  }

  function probeWith(availableModels: string[], defaultModel: string) {
    return {
      cliInstalled: true,
      os: 'linux',
      arch: 'x86_64',
      installSupported: true,
      protocol: {
        protocolVersion: 4,
        cliVersion: '1.2.3',
        os: 'linux',
        arch: 'x86_64',
        capabilities: [...BASE_DISPATCH_CAPABILITIES],
        modelConfigured: true,
        availableModels,
        defaultModel,
      },
    };
  }

  async function mount() {
    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={target}
          onClose={vi.fn()}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.modalOnClose = null;
    mocks.confirmWarning.mockResolvedValue(true);
    mocks.getFreshConfig.mockResolvedValue(undefined);
    mocks.resolveRevision.mockResolvedValue('a'.repeat(40));
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('reports parity with this device instead of an opaque config id', async () => {
    mocks.probeTarget.mockResolvedValue(
      probeWith(['model_1', 'model_2'], 'model_2'),
    );
    mocks.getConfig.mockResolvedValue([
      localModel('model_1', 'claude-haiku'),
      localModel('model_2', 'claude-opus'),
    ]);

    await mount();

    expect(container.textContent).toContain('dispatch.modelMatchesLocal');
    expect(container.textContent).not.toContain('dispatch.modelDiffersFromLocal');
    // The id itself must never be what the user is asked to read.
    expect(container.textContent).not.toContain('model_2');
  });

  it('reports the target model count when the catalogs differ', async () => {
    mocks.probeTarget.mockResolvedValue(probeWith(['model_1'], 'model_1'));
    mocks.getConfig.mockResolvedValue([
      localModel('model_1', 'claude-haiku'),
      localModel('model_2', 'claude-opus'),
    ]);

    await mount();

    expect(container.textContent).toContain('dispatch.modelDiffersFromLocal');
    expect(container.textContent).not.toContain('dispatch.modelMatchesLocal');
  });

  it('claims no parity when the local catalog cannot be read', async () => {
    mocks.probeTarget.mockResolvedValue(probeWith(['model_1'], 'model_1'));
    mocks.getConfig.mockRejectedValue(new Error('config unavailable'));

    await mount();

    expect(container.textContent).toContain('dispatch.modelReadyCount');
    expect(container.textContent).not.toContain('dispatch.modelMatchesLocal');
    expect(container.textContent).not.toContain('dispatch.modelDiffersFromLocal');
  });
});
