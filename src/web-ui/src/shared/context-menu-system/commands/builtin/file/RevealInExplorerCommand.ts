import { BaseCommand } from '../../BaseCommand';
import type { CommandResult } from '../../../types/command.types';
import { ContextType, type MenuContext } from '../../../types/context.types';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { notificationService } from '@/shared/notification-system/services/NotificationService';
import { isRemoteWorkspace } from '@/shared/types';
import { createLogger } from '@/shared/utils/logger';
import { hasNonFileUriScheme } from '@/shared/utils/pathUtils';
import { getActiveSurfaceId } from '@/infrastructure/peer-device/deviceSurface';

const log = createLogger('RevealInExplorerCommand');

function getContextFilePath(context: MenuContext): string | undefined {
  if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
    return context.filePath;
  }

  if (context.type === ContextType.TAB) {
    return context.filePath;
  }

  return undefined;
}

/** Shared by menu availability and execution; native actions never use remote paths. */
export function canRevealInExplorer(context: MenuContext): boolean {
  const filePath = getContextFilePath(context);
  return Boolean(filePath?.trim())
    && !hasNonFileUriScheme(filePath || '')
    && (context.resourceScope
      ? context.resourceScope.surfaceId === getActiveSurfaceId() && !context.resourceScope.remoteConnectionId
      : !isRemoteWorkspace(workspaceManager.getState().currentWorkspace));
}

export class RevealInExplorerCommand extends BaseCommand {
  constructor() {
    super({
      id: 'file.reveal-in-explorer',
      label: i18nService.t('common:file.reveal'),
      description: i18nService.t('common:file.revealDescription'),
      icon: 'FolderOpen',
      category: 'file'
    });
  }

  canExecute(context: MenuContext): boolean {
    return canRevealInExplorer(context);
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    const filePath = getContextFilePath(context);
    if (!filePath || !this.canExecute(context)) {
      return this.failure(i18nService.t('errors:file.revealFailed'));
    }

    try {
      // The command owns this effect, independently of mounted file-tree panels.
      await workspaceAPI.revealInExplorer(filePath);

      return this.success(i18nService.t('common:file.revealOpening'), { path: filePath });
    } catch (error) {
      const cause = error instanceof Error ? error : new Error(String(error));
      const message = i18nService.t('errors:file.revealFailed');
      log.error('Failed to reveal path in file explorer', { filePath, error: cause });
      notificationService.error(cause.message, { title: message });
      return this.failure(message, cause);
    }
  }
}

