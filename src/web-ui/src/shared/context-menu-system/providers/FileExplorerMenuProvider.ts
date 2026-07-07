 

import { IMenuProvider } from '../types/provider.types';
import { MenuItem } from '../types/menu.types';
import { MenuContext, ContextType, FileNodeContext } from '../types/context.types';
import { commandExecutor } from '../commands/CommandExecutor';
import { globalEventBus } from '../../../infrastructure/event-bus';
import { i18nService } from '../../../infrastructure/i18n';
import { workspaceManager } from '../../../infrastructure/services/business/workspaceManager';
import { isRemoteWorkspace } from '../../../shared/types';
import { addFileMentionToChat } from '@/shared/utils/chatContext';
import { dirnameAbsolutePath } from '@/shared/utils/pathUtils';
import { isHtmlFilePath } from '@/shared/utils/htmlFilePreview';

const PASTE_SHORTCUT = /Mac|iPhone|iPad|iPod/.test(navigator.userAgent) ? 'Cmd+V' : 'Ctrl+V';

const ARCHIVE_EXTENSIONS = [
  '.zip', '.tar.gz', '.tgz', '.tar',
  '.tar.bz2', '.tbz2', '.tar.xz', '.txz', '.tar.zst', '.tzst',
];

function isArchiveFile(filePath: string): boolean {
  const lower = filePath.toLowerCase();
  return ARCHIVE_EXTENSIONS.some((ext) => lower.endsWith(ext));
}

export class FileExplorerMenuProvider implements IMenuProvider {
  readonly id = 'file-explorer';
  readonly name = i18nService.t('common:contextMenu.fileExplorerMenu.name');
  readonly description = i18nService.t('common:contextMenu.fileExplorerMenu.description');
  readonly priority = 80;

  matches(context: MenuContext): boolean {
    
    if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
      return true;
    }
    
    
    if (context.type === ContextType.EMPTY_SPACE) {
      const emptyContext = context as any;
      return emptyContext.area === 'file-explorer';
    }
    
    return false;
  }

  async getMenuItems(context: MenuContext): Promise<MenuItem[]> {
    const items: MenuItem[] = [];
    const localFileActionsDisabled = isRemoteWorkspace(workspaceManager.getState().currentWorkspace);

    if (context.type === ContextType.EMPTY_SPACE) {
      const emptyContext = context as any;
      
      
      const workspaceRoot = this.findWorkspaceRoot(emptyContext.targetElement);
      
      if (workspaceRoot) {
        const parentPath = workspaceRoot; 
        
        items.push({
          id: 'file-new-file',
          label: i18nService.t('common:file.newFile'),
          icon: 'FilePlus',
          onClick: () => {
            globalEventBus.emit('file:new-file', { parentPath });
          }
        });

        items.push({
          id: 'file-new-folder',
          label: i18nService.t('common:file.newFolder'),
          icon: 'FolderPlus',
          onClick: () => {
            globalEventBus.emit('file:new-folder', { parentPath });
          }
        });

        items.push({
          id: 'file-separator-paste',
          label: '',
          separator: true
        });

        
        items.push({
          id: 'file-paste',
          label: i18nService.t('common:actions.paste'),
          icon: 'Clipboard',
          shortcut: PASTE_SHORTCUT,
          onClick: async () => {
            globalEventBus.emit('file:paste', { targetDirectory: parentPath });
          }
        });
      }
      
      return items;
    }
    
    
    const fileContext = context as FileNodeContext;
    const isDirectory = fileContext.isDirectory;
    const isReadOnly = fileContext.isReadOnly;

    
    if (!isDirectory) {
      items.push({
        id: 'file-open',
        label: i18nService.t('common:actions.open'),
        icon: 'FileText',
        onClick: () => {
          globalEventBus.emit('file:open', { path: fileContext.filePath });
        }
      });

      if (isHtmlFilePath(fileContext.filePath)) {
        items.push({
          id: 'file-open-html-in-browser',
          label: i18nService.t('common:file.openInBrowser'),
          icon: 'ExternalLink',
          command: 'file.open-html-in-browser',
          disabled: localFileActionsDisabled,
          onClick: async (ctx) => {
            await commandExecutor.execute('file.open-html-in-browser', ctx);
          }
        });
      }

    }

    // Download (available for both files and directories)
    items.push({
      id: 'file-download',
      label: i18nService.t('common:file.download'),
      icon: 'Download',
      onClick: () => {
        globalEventBus.emit('file:download', { path: fileContext.filePath, isDirectory });
      }
    });

    items.push({
      id: 'file-separator-1',
      label: '',
      separator: true
    });

    
    if (!isReadOnly) {

      // Compress: available for both files and directories.
      items.push({
        id: 'file-compress',
        label: i18nService.t('panels/files:archive.compress'),
        icon: 'Archive',
        onClick: () => {
          globalEventBus.emit('file:compress', { path: fileContext.filePath, isDirectory });
        }
      });

      // Decompress: only for archive files (not directories).
      if (!isDirectory && isArchiveFile(fileContext.filePath)) {
        items.push({
          id: 'file-decompress',
          label: i18nService.t('panels/files:archive.decompress'),
          icon: 'ArchiveRestore',
          onClick: () => {
            globalEventBus.emit('file:decompress', { path: fileContext.filePath });
          }
        });
      }

      items.push({
        id: 'file-separator-archive',
        label: '',
        separator: true
      });

      if (isDirectory) {
        items.push({
          id: 'file-new',
          label: i18nService.t('common:actions.new'),
          icon: 'Plus',
          submenu: [
            {
              id: 'file-new-file',
              label: i18nService.t('common:file.file'),
              icon: 'FilePlus',
              command: 'file.new-file',
              onClick: async (ctx) => {
                await commandExecutor.execute('file.new-file', ctx);
              }
            },
            {
              id: 'file-new-folder',
              label: i18nService.t('common:file.folder'),
              icon: 'FolderPlus',
              command: 'file.new-folder',
              onClick: async (ctx) => {
                await commandExecutor.execute('file.new-folder', ctx);
              }
            }
          ]
        });
      }

      
      items.push({
        id: 'file-rename',
        label: i18nService.t('common:file.rename'),
        icon: 'Edit',
        shortcut: 'F2',
        command: 'file.rename',
        onClick: async (ctx) => {
          await commandExecutor.execute('file.rename', ctx);
        }
      });

      
      items.push({
        id: 'file-delete',
        label: i18nService.t('common:file.delete'),
        icon: 'Trash2',
        command: 'file.delete',
        onClick: async (ctx) => {
          await commandExecutor.execute('file.delete', ctx);
        }
      });

      items.push({
        id: 'file-separator-3',
        label: '',
        separator: true
      });

      
      
      
      const targetDirectory = isDirectory
        ? fileContext.filePath
        : this.getParentDirectory(fileContext.filePath);

      items.push({
        id: 'file-paste',
        label: i18nService.t('common:actions.paste'),
        icon: 'Clipboard',
        shortcut: PASTE_SHORTCUT,
        onClick: async () => {
          globalEventBus.emit('file:paste', { targetDirectory });
        }
      });

      items.push({
        id: 'file-separator-paste',
        label: '',
        separator: true
      });
    }

    
    items.push({
      id: 'file-add-to-chat',
      label: i18nService.t('common:editor.addToChat'),
      icon: 'MessageSquarePlus',
      onClick: () => {
        addFileMentionToChat(
          {
            path: fileContext.filePath,
            name: fileContext.fileName,
            isDirectory,
          },
          fileContext.workspacePath,
        );
      }
    });

    items.push({
      id: 'file-separator-chat',
      label: '',
      separator: true
    });

    items.push({
      id: 'file-copy-path',
      label: i18nService.t('common:file.copyPath'),
      icon: 'Copy',
      command: 'file.copy-path',
      onClick: async (ctx) => {
        await commandExecutor.execute('file.copy-path', ctx);
      }
    });

    items.push({
      id: 'file-copy-relative-path',
      label: i18nService.t('common:file.copyRelativePath'),
      icon: 'Copy',
      command: 'file.copy-relative-path',
      onClick: async (ctx) => {
        await commandExecutor.execute('file.copy-relative-path', ctx);
      }
    });

    items.push({
      id: 'file-reveal',
      label: i18nService.t('common:file.reveal'),
      icon: 'FolderOpen',
      command: 'file.reveal-in-explorer',
      disabled: localFileActionsDisabled,
      onClick: async (ctx) => {
        await commandExecutor.execute('file.reveal-in-explorer', ctx);
      }
    });

    return items;
  }

  isEnabled(): boolean {
    return true;
  }

   
  private findWorkspaceRoot(element: HTMLElement | null): string | null {
    let current = element;
    
    while (current && current !== document.body) {
      const workspaceRoot = current.getAttribute('data-workspace-root');
      if (workspaceRoot) {
        return workspaceRoot;
      }
      current = current.parentElement;
    }
    
    return null;
  }

   
  private getParentDirectory(filePath: string): string {
    return dirnameAbsolutePath(filePath);
  }
}
