// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DispatchInstallDialog } from './DispatchInstallDialog';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  probeTarget: vi.fn(),
  installCliSourceStart: vi.fn(),
  installCliPoll: vi.fn(),
  installCliCancel: vi.fn(),
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
    installCliSourceStart: mocks.installCliSourceStart,
    installCliPoll: mocks.installCliPoll,
    installCliCancel: mocks.installCliCancel,
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

describe('DispatchInstallDialog installation lifecycle', () => {
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
    mocks.installCliCancel.mockResolvedValue(undefined);
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

    expect(container.textContent).toContain('dispatch.installAutomaticTitle');
    expect(container.textContent).toContain('1.2.3');
    expect(container.textContent).toContain('abc123');
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

  it('offers a source build only when the target can actually run one', async () => {
    // A target no published binary fits: the release install is not offered,
    // and the source build is gated on its prerequisites rather than failing
    // partway through.
    mocks.probeTarget.mockResolvedValue({
      cliInstalled: false,
      os: 'linux',
      arch: 'x86_64',
      installSupported: false,
      prebuiltIncompatible: 'target uses musl libc',
      sourceBuild: {
        supported: false,
        blockers: ['no cargo on the target'],
        gitRef: 'v1.2.3',
      },
    });

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{ kind: 'ssh', connectionId: 'ssh-1', displayName: 'alpine-host' }}
          onClose={vi.fn()}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
    });

    expect(container.textContent).toContain('target uses musl libc');
    expect(container.textContent).toContain('no cargo on the target');
    const buttons = () => Array.from(container.querySelectorAll('button'));
    expect(
      buttons().find(button => button.textContent?.includes('dispatch.installConfirm')),
      'a prebuilt install that cannot work must not be offered',
    ).toBeUndefined();
    const blocked = buttons()
      .find(button => button.textContent?.includes('dispatch.sourceBuildConfirm'));
    expect(blocked?.disabled).toBe(true);

    // Same target once a toolchain is present.
    mocks.probeTarget.mockResolvedValue({
      cliInstalled: false,
      os: 'linux',
      arch: 'x86_64',
      installSupported: false,
      prebuiltIncompatible: 'target uses musl libc',
      sourceBuild: { supported: true, blockers: [], gitRef: 'v1.2.3', cargoVersion: '1.90.0' },
    });
    mocks.installCliSourceStart.mockResolvedValue({
      scriptPath: '/tmp/install-bitfun.sh',
      version: '1.2.3',
      target: 'linux x86_64',
      url: 'https://github.com/GCWing/BitFun.git',
      sha256: '',
    });
    mocks.installCliPoll.mockResolvedValue({ cursor: 1, output: '', status: 'failed' });

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

    const ready = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.sourceBuildConfirm'));
    expect(ready?.disabled).toBe(false);
    await act(async () => {
      ready?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.confirmWarning).toHaveBeenCalled();
    expect(mocks.installCliSourceStart).toHaveBeenCalledWith('ssh-1');
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
        capabilities: [
          'persistent_jobs',
          'cursor_events',
          'detached_worker',
          'frontend_event_projection',
          'workspace_serialization',
          'dispatch_worker_cli_profile',
          'workspace_git_worktree',
          'workspace_git_bundle_upload',
          'workspace_git_sync',
          'approval_remote',
          'per_turn_options',
          'session_query',
          'inline_attachments',
        ],
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

  it('cancels an acknowledged installer when the parent closes the dialog during polling', async () => {
    const poll = createDeferred<{
      cursor: number;
      output: string;
      status: 'running';
    }>();
    mocks.probeTarget.mockResolvedValue({
      cliInstalled: false,
      os: 'linux',
      arch: 'x86_64',
      installSupported: false,
      prebuiltIncompatible: 'target uses musl libc',
      sourceBuild: {
        supported: true,
        blockers: [],
        gitRef: 'v1.2.3',
        cargoVersion: '1.90.0',
      },
    });
    mocks.installCliSourceStart.mockResolvedValue({
      scriptPath: '/tmp/install-bitfun.sh',
      version: '1.2.3',
      target: 'linux x86_64',
      url: 'https://github.com/GCWing/BitFun.git',
      sha256: '',
    });
    mocks.installCliPoll.mockReturnValue(poll.promise);
    const target = {
      kind: 'ssh' as const,
      connectionId: 'ssh-1',
      displayName: 'build-host',
    };

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
    });

    const installButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.sourceBuildConfirm'));
    await act(async () => {
      installButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.installCliSourceStart).toHaveBeenCalledTimes(1);
    expect(mocks.installCliPoll).toHaveBeenCalledTimes(1);

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open={false}
          target={target}
          onClose={vi.fn()}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
    });
    expect(mocks.installCliCancel).toHaveBeenCalledTimes(1);
    expect(mocks.installCliCancel).toHaveBeenCalledWith('ssh-1');

    await act(async () => {
      poll.resolve({
        cursor: 1,
        output: 'still running',
        status: 'running',
      });
      await Promise.resolve();
    });
    expect(mocks.installCliPoll).toHaveBeenCalledTimes(1);
    expect(mocks.installCliCancel).toHaveBeenCalledTimes(1);
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
        capabilities: [
          'persistent_jobs',
          'cursor_events',
          'detached_worker',
          'frontend_event_projection',
          'workspace_serialization',
          'workspace_git_worktree',
          'workspace_git_bundle_upload',
          'workspace_git_sync',
          'dispatch_worker_cli_profile',
          'per_turn_options',
          'session_query',
          'inline_attachments',
        ],
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
    });
  }

  beforeEach(() => {
    vi.clearAllMocks();
    modelConfigured = false;
    mocks.modalOnClose = null;
    mocks.probeTarget.mockImplementation(async () => probeResult());
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

  it('keeps model sync available after the target reports a usable model', async () => {
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
    expect(syncButton()).toBeDefined();
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

  it('discards a late sync acknowledgement after the dialog closes', async () => {
    const sync = createDeferred<void>();
    mocks.syncModelConfig.mockReturnValue(sync.promise);
    await mount();

    await act(async () => {
      syncButton()?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.syncModelConfig).toHaveBeenCalledTimes(1);
    const probesBeforeClose = mocks.probeTarget.mock.calls.length;

    await act(async () => {
      mocks.modalOnClose?.();
      await Promise.resolve();
    });

    await act(async () => {
      modelConfigured = true;
      sync.resolve(undefined);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.probeTarget.mock.calls.length).toBe(probesBeforeClose);
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
        capabilities: [
          'persistent_jobs',
          'cursor_events',
          'detached_worker',
          'frontend_event_projection',
          'workspace_serialization',
          'workspace_git_worktree',
          'workspace_git_bundle_upload',
          'workspace_git_sync',
          'dispatch_worker_cli_profile',
          'per_turn_options',
          'session_query',
          'inline_attachments',
        ],
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
