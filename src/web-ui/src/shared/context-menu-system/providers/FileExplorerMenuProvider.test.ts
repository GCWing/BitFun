import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ContextType, type FileNodeContext } from '../types/context.types';

const mocks = vi.hoisted(() => ({
  emit: vi.fn(),
  execute: vi.fn(),
  openFileInBestTarget: vi.fn(),
}));

vi.mock('../commands/CommandExecutor', () => ({
  commandExecutor: { execute: mocks.execute },
}));

vi.mock('../../../infrastructure/event-bus', () => ({
  globalEventBus: { emit: mocks.emit },
}));

vi.mock('../../../infrastructure/i18n', () => ({
  i18nService: { t: (key: string) => key },
}));

vi.mock('../../../infrastructure/services/business/workspaceManager', () => ({
  workspaceManager: { getState: () => ({ currentWorkspace: null }) },
}));

vi.mock('@/shared/utils/chatContext', () => ({ addFileMentionToChat: vi.fn() }));
vi.mock('@/shared/utils/tabUtils', () => ({
  openFileInBestTarget: mocks.openFileInBestTarget,
}));

import { FileExplorerMenuProvider } from './FileExplorerMenuProvider';

function fileContext(fileName: string): FileNodeContext {
  return {
    type: ContextType.FILE_NODE,
    filePath: `E:/project/${fileName}`,
    fileName,
    workspacePath: 'E:/project',
    isDirectory: false,
    isReadOnly: false,
    targetElement: null,
  } as FileNodeContext;
}

describe('FileExplorerMenuProvider HTML actions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each(['index.html', 'legacy.HTM'])('offers explicit source, integrated, and system targets for %s', async (fileName) => {
    const context = fileContext(fileName);
    const items = await new FileExplorerMenuProvider().getMenuItems(context);
    const open = items.find(item => item.id === 'file-open');
    const integrated = items.find(item => item.id === 'file-open-html-in-integrated-browser');
    const system = items.find(item => item.id === 'file-open-html-in-browser');

    expect(open).toMatchObject({ label: 'common:actions.open', icon: 'FileText' });
    expect(integrated).toMatchObject({
      label: 'common:file.openInIntegratedBrowser',
      icon: 'PanelRightOpen',
    });
    expect(system).toMatchObject({
      label: 'common:file.openInSystemBrowser',
      icon: 'ExternalLink',
      command: 'file.open-html-in-browser',
      disabled: false,
    });

    open?.onClick?.(context);
    expect(mocks.openFileInBestTarget).toHaveBeenLastCalledWith(expect.objectContaining({
      filePath: context.filePath,
      editorType: 'code-editor',
    }));

    integrated?.onClick?.(context);
    expect(mocks.openFileInBestTarget).toHaveBeenLastCalledWith(expect.objectContaining({
      filePath: context.filePath,
      editorType: 'html-preview',
    }));

    await system?.onClick?.(context);
    expect(mocks.execute).toHaveBeenCalledWith('file.open-html-in-browser', context);
    expect(mocks.emit).not.toHaveBeenCalledWith('file:open', expect.anything());
  });

  it('keeps the ordinary open event for non-HTML files', async () => {
    const context = fileContext('notes.txt');
    const items = await new FileExplorerMenuProvider().getMenuItems(context);

    await items.find(item => item.id === 'file-open')?.onClick?.(context);

    expect(mocks.emit).toHaveBeenCalledWith('file:open', { path: context.filePath });
    expect(mocks.openFileInBestTarget).not.toHaveBeenCalled();
  });
});
