 

import { MenuContext, ContextType } from '../types/context.types';
import { i18nService } from '@/infrastructure/i18n';

 
export function getContextDescription(context: MenuContext): string {
  switch (context.type) {
    case ContextType.SELECTION:
      return i18nService.t('components:contextSystem.contextTypes.selection');
    case ContextType.FILE_NODE:
      return i18nService.t('components:contextSystem.contextTypes.file');
    case ContextType.FOLDER_NODE:
      return i18nService.t('components:contextSystem.contextTypes.folder');
    case ContextType.EDITOR:
      return i18nService.t('components:contextSystem.contextTypes.editor');
    case ContextType.FLOWCHAT:
      return i18nService.t('components:contextSystem.contextTypes.flowChat');
    case ContextType.FLOWCHAT_TOOL_CARD:
      return i18nService.t('components:contextSystem.contextTypes.flowChatToolCard');
    case ContextType.FLOWCHAT_TEXT_BLOCK:
      return i18nService.t('components:contextSystem.contextTypes.flowChatTextBlock');
    case ContextType.TAB:
      return i18nService.t('components:contextSystem.contextTypes.tab');
    case ContextType.PANEL_HEADER:
      return i18nService.t('components:contextSystem.contextTypes.panelHeader');
    case ContextType.EMPTY_SPACE:
      return i18nService.t('components:contextSystem.contextTypes.emptySpace');
    case ContextType.CUSTOM:
      return i18nService.t('components:contextSystem.contextTypes.custom');
    default:
      return i18nService.t('components:contextSystem.contextTypes.unknown');
  }
}

 
export function hasTextSelection(context: MenuContext): boolean {
  return 'selectedText' in context && !!context.selectedText;
}

 
export function isEditable(context: MenuContext): boolean {
  if (context.type === ContextType.SELECTION) {
    return (context as any).isEditable;
  }
  if (context.type === ContextType.EDITOR) {
    return !(context as any).isReadOnly;
  }
  return false;
}

 
export function getFilePath(context: MenuContext): string | undefined {
  if ('filePath' in context) {
    return context.filePath as string;
  }
  return undefined;
}

 
export function matchesContextType(context: MenuContext, types: ContextType | ContextType[]): boolean {
  const typeArray = Array.isArray(types) ? types : [types];
  return typeArray.includes(context.type);
}

 
export function extractMetadata(context: MenuContext): Record<string, any> {
  const metadata: Record<string, any> = {
    type: context.type,
    timestamp: context.timestamp
  };

  
  if ('selectedText' in context) {
    metadata.hasSelection = !!context.selectedText;
    metadata.selectionLength = context.selectedText?.length || 0;
  }

  if ('filePath' in context) {
    metadata.filePath = context.filePath;
  }

  if ('isDirectory' in context) {
    metadata.isDirectory = context.isDirectory;
  }

  return metadata;
}

