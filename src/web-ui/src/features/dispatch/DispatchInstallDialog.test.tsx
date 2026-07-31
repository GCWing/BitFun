// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DispatchInstallDialog } from './DispatchInstallDialog';
import type { DispatchInstallStart } from './types';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  probeTarget: vi.fn(),
  installCliStart: vi.fn(),
  installCliSourceStart: vi.fn(),
  installCliPoll: vi.fn(),
  installCliCancel: vi.fn(),
  syncModelConfig: vi.fn(),
  confirmWarning: vi.fn(),
  modalOnClose: null as (() => void) | null,
  modalLifecycleProps: null as {
    closeOnOverlayClick?: boolean;
    showCloseButton?: boolean;
  } | null,
}));

vi.mock('./dispatchApi', () => ({
  dispatchApi: {
    probeTarget: mocks.probeTarget,
    installCliStart: mocks.installCliStart,
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
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('cancels a late installer acknowledgement after the dialog closes during start', async () => {
    const start = createDeferred<DispatchInstallStart>();
    mocks.installCliStart.mockReturnValue(start.promise);
    const onClose = vi.fn();

    await act(async () => {
      root.render(
        <DispatchInstallDialog
          open
          target={{
            kind: 'ssh',
            connectionId: 'ssh-1',
            displayName: 'build-host',
          }}
          onClose={onClose}
          onReady={vi.fn()}
        />,
      );
      await Promise.resolve();
    });

    const installButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.installConfirm'));
    expect(installButton).toBeDefined();

    await act(async () => {
      installButton?.click();
      await Promise.resolve();
    });
    expect(mocks.installCliStart).toHaveBeenCalledTimes(1);
    expect(mocks.modalLifecycleProps).toEqual({
      closeOnOverlayClick: true,
      showCloseButton: true,
    });
    const snapshotButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.deliverySnapshot'));
    expect(snapshotButton?.disabled).toBe(true);
    expect(snapshotButton?.textContent).toContain('dispatch.deliverySnapshotUnavailable');
    const cancelButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent === 'dispatch.cancel');
    expect(cancelButton?.disabled).toBe(false);

    await act(async () => {
      mocks.modalOnClose?.();
      await Promise.resolve();
    });
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(mocks.installCliCancel).toHaveBeenCalledTimes(1);

    await act(async () => {
      start.resolve({
        scriptPath: '/tmp/install-bitfun.sh',
        version: '1.2.3',
        target: 'x86_64-unknown-linux-gnu',
        url: 'https://example.test/bitfun',
        sha256: 'abc123',
      });
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.installCliCancel).toHaveBeenCalledTimes(2);
    expect(mocks.installCliCancel).toHaveBeenLastCalledWith('ssh-1');
    expect(mocks.installCliPoll).not.toHaveBeenCalled();
    expect(container.querySelector('pre')).toBeNull();
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
    mocks.installCliPoll.mockResolvedValue({ cursor: 0, output: '', status: 'running' });

    const checkButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.check'));
    await act(async () => {
      checkButton?.click();
      await Promise.resolve();
    });

    const ready = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.sourceBuildConfirm'));
    expect(ready?.disabled).toBe(false);
    await act(async () => {
      ready?.click();
      await Promise.resolve();
    });
    expect(mocks.confirmWarning).toHaveBeenCalled();
    expect(mocks.installCliSourceStart).toHaveBeenCalledWith('ssh-1');
  });

  it('names where snapshot results stay, since nothing is synced back', async () => {
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

    const snapshot = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.deliverySnapshot'));
    await act(async () => {
      snapshot?.click();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.snapshotResultLocationHint');
  });

  it('defaults an unbound target to a source snapshot and preserves target model facts', async () => {
    const onReady = vi.fn();
    mocks.probeTarget.mockResolvedValue({
      cliInstalled: true,
      os: 'linux',
      arch: 'x86_64',
      installSupported: false,
      protocol: {
        protocolVersion: 2,
        cliVersion: '1.2.3',
        os: 'linux',
        arch: 'x86_64',
        capabilities: [
          'persistent_jobs',
          'cursor_events',
          'detached_worker',
          'frontend_event_projection',
          'workspace_serialization',
          'workspace_snapshot_exact',
          'workspace_snapshot_chunked',
          'approval_remote',
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

    const sourceSnapshot = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('dispatch.deliverySourceSnapshot'));
    expect(sourceSnapshot?.getAttribute('aria-checked')).toBe('true');

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
    });
    expect(onReady).toHaveBeenCalledWith(expect.objectContaining({
      workspaceDelivery: {
        kind: 'snapshot-source',
        sourceWorkspacePath: '/home/me/project',
      },
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
    mocks.installCliStart.mockResolvedValue({
      scriptPath: '/tmp/install-bitfun.sh',
      version: '1.2.3',
      target: 'x86_64-unknown-linux-gnu',
      url: 'https://example.test/bitfun',
      sha256: 'abc123',
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
      .find(button => button.textContent?.includes('dispatch.installConfirm'));
    await act(async () => {
      installButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.installCliStart).toHaveBeenCalledTimes(1);
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
        protocolVersion: 2,
        cliVersion: '1.2.3',
        os: 'linux',
        arch: 'x86_64',
        capabilities: [
          'persistent_jobs',
          'cursor_events',
          'detached_worker',
          'frontend_event_projection',
          'workspace_serialization',
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
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('offers the sync only while the target CLI answers without a usable model', async () => {
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
