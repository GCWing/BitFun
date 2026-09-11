import { BaseCommand } from '../../BaseCommand';
import { CommandResult } from '../../../types/command.types';
import { ContextType, FileNodeContext, MenuContext, TabContext } from '../../../types/context.types';
import { i18nService } from '@/infrastructure/i18n';
import { canRevealInExplorer } from './RevealInExplorerCommand';
import { isHtmlFilePath, openHtmlFileInExternalBrowser } from '@/shared/utils/htmlFilePreview';

function getContextFilePath(context: MenuContext): string | undefined {
  if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
    return (context as FileNodeContext).filePath;
  }

  if (context.type === ContextType.TAB) {
    return (context as TabContext).filePath;
  }

  return undefined;
}

export class OpenHtmlInBrowserCommand extends BaseCommand {
  constructor() {
    const t = i18nService.getT();
    super({
      id: 'file.open-html-in-browser',
      label: t('common:file.openInSystemBrowser'),
      description: t('common:file.openInBrowserDescription'),
      icon: 'ExternalLink',
      category: 'file'
    });
  }

  canExecute(context: MenuContext): boolean {
    const filePath = getContextFilePath(context);
    const htmlFile = Boolean(filePath && isHtmlFilePath(filePath));
    return htmlFile && canRevealInExplorer(context);
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const t = i18nService.getT();
      const filePath = getContextFilePath(context);

      if (!filePath || !this.canExecute(context)) {
        return this.failure(t('errors:file.openInBrowserFailed'));
      }

      await openHtmlFileInExternalBrowser(filePath);
      return this.success(t('common:file.openInBrowserOpening'), { path: filePath });
    } catch (error) {
      const t = i18nService.getT();
      return this.failure(t('errors:file.openInBrowserFailed'), error as Error);
    }
  }
}
