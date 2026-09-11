import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { globalEventBus } from '@/infrastructure/event-bus';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';
import { CommandExecutor } from '../../CommandExecutor';
import { CommandRegistry } from '../../CommandRegistry';
import { ContextType, type FileNodeContext, type TabContext } from '../../../types/context.types';
import { canRevealInExplorer, RevealInExplorerCommand } from './RevealInExplorerCommand';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  notifyError: vi.fn(),
  currentWorkspace: null as WorkspaceInfo | null,
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { invoke: mocks.invoke },
}));

vi.mock('@/infrastructure/services/business/workspaceManager', () => ({
  workspaceManager: { getState: () => ({ currentWorkspace: mocks.currentWorkspace }) },
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: { t: (key: string) => key },
}));

vi.mock('@/shared/notification-system/services/NotificationService', () => ({
  notificationService: { error: mocks.notifyError },
}));

function fileContext(
  type: ContextType.TAB | ContextType.FILE_NODE | ContextType.FOLDER_NODE = ContextType.TAB,
  filePath: string | undefined = 'E:/project with spaces/notes %20.txt',
): FileNodeContext | TabContext {
  return {
    type,
    filePath,
    workspacePath: 'E:/project with spaces',
    tabId: 'file-tab',
    tabTitle: 'notes %20.txt',
    fileName: 'notes %20.txt',
    isDirectory: type === ContextType.FOLDER_NODE,
  } as FileNodeContext | TabContext;
}

describe('RevealInExplorerCommand', () => {
  let command: RevealInExplorerCommand;
  let executor: CommandExecutor;
  const unsubscribers: Array<() => void> = [];

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.currentWorkspace = null;
    mocks.invoke.mockResolvedValue(undefined);
    command = new RevealInExplorerCommand();
    const registry = new CommandRegistry();
    registry.register(command);
    executor = new CommandExecutor(registry, { enableHistory: false });
  });

  it('uses the file origin when resource navigation differs from the active workspace', () => {
    const context = fileContext(ContextType.FILE_NODE);
    context.resourceScope = { surfaceId: 'local', workspaceId: 'remote', workspacePath: '/repo', remoteConnectionId: 'ssh-b' };
    expect(canRevealInExplorer(context)).toBe(false);
    mocks.currentWorkspace = { workspaceKind: WorkspaceKind.Remote, connectionId: 'ssh-a' } as WorkspaceInfo;
    context.resourceScope = { surfaceId: 'local', workspaceId: 'local', workspacePath: 'E:/repo' };
    expect(canRevealInExplorer(context)).toBe(true);
    context.resourceScope = { ...context.resourceScope, surfaceId: 'other-peer' };
    expect(canRevealInExplorer(context)).toBe(false);
  });

  afterEach(() => {
    unsubscribers.splice(0).forEach(unsubscribe => unsubscribe());
  });

  it.each([ContextType.TAB, ContextType.FILE_NODE, ContextType.FOLDER_NODE] as const)(
    'opens a %s path through the platform API without a mounted file panel',
    async (type) => {
      const context = fileContext(type);

      const result = await executor.execute(command.id, context);

      expect(result).toMatchObject({ success: true, data: { path: context.filePath } });
      expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith('reveal_in_explorer', {
        request: { path: context.filePath },
      });
      expect(mocks.notifyError).not.toHaveBeenCalled();
    },
  );

  it('does not broadcast the native action to mounted panel listeners', async () => {
    const firstPanel = vi.fn();
    const secondPanel = vi.fn();
    unsubscribers.push(
      globalEventBus.on('file:reveal', firstPanel),
      globalEventBus.on('file:reveal', secondPanel),
    );

    await executor.execute(command.id, fileContext());

    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(firstPanel).not.toHaveBeenCalled();
    expect(secondPanel).not.toHaveBeenCalled();
  });

  it('reports success only after the platform acknowledges the request', async () => {
    let complete!: () => void;
    mocks.invoke.mockImplementationOnce(() => new Promise<void>(resolve => { complete = resolve; }));
    const success = vi.fn();
    unsubscribers.push(globalEventBus.on('command:success', success));
    const pending = executor.execute(command.id, fileContext());

    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(1));
    expect(success).not.toHaveBeenCalled();

    complete();
    expect((await pending).success).toBe(true);
    expect(success).toHaveBeenCalledTimes(1);
  });

  it.each([
    new Error('Path does not exist'),
    'Host does not support reveal_in_explorer',
  ])('returns a failure and one visible notification for a platform rejection: %s', async (error) => {
    mocks.invoke.mockRejectedValueOnce(error);
    const success = vi.fn();
    const failure = vi.fn();
    unsubscribers.push(
      globalEventBus.on('command:success', success),
      globalEventBus.on('command:failure', failure),
    );

    const result = await executor.execute(command.id, fileContext());
    const message = error instanceof Error ? error.message : error;

    expect(result).toMatchObject({ success: false, message: 'errors:file.revealFailed' });
    expect(result.error?.message).toBe(message);
    expect(mocks.notifyError).toHaveBeenCalledExactlyOnceWith(message, {
      title: 'errors:file.revealFailed',
    });
    expect(success).not.toHaveBeenCalled();
    expect(failure).toHaveBeenCalledTimes(1);
  });

  it('rechecks remote workspace restrictions even when execute is called directly', async () => {
    const context = fileContext(ContextType.TAB, '/srv/project/notes.txt');
    expect(command.canExecute(context)).toBe(true);
    mocks.currentWorkspace = { workspaceKind: WorkspaceKind.Remote } as WorkspaceInfo;

    expect(canRevealInExplorer(context)).toBe(false);
    expect(command.canExecute(context)).toBe(false);
    expect((await command.execute(context)).success).toBe(false);
    expect(mocks.invoke).not.toHaveBeenCalled();
  });

  it.each(['', '   ', 'openbitfun://runtime/artifacts/note', 'https://example.com/note'])(
    'disables non-filesystem targets without invoking the host: %s',
    async (filePath) => {
      const context = fileContext(ContextType.TAB, filePath);

      expect(canRevealInExplorer(context)).toBe(false);
      expect((await executor.execute(command.id, context)).success).toBe(false);
      expect((await command.execute(context)).success).toBe(false);
      expect(mocks.invoke).not.toHaveBeenCalled();
    },
  );
});
