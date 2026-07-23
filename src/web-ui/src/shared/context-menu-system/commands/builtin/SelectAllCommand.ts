 

import { BaseCommand } from '../BaseCommand';
import { CommandResult } from '../../types/command.types';
import { MenuContext, ContextType, EditorContext } from '../../types/context.types';
import { MonacoHelper } from '@/shared/helpers/MonacoHelper';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('SelectAllCommand');

export class SelectAllCommand extends BaseCommand {
  constructor() {
    const t = i18nService.getT();
    super({
      id: 'select-all',
      label: t('common:actions.selectAll'),
      description: t('common:contextMenu.descriptions.selectAll'),
      icon: 'SelectAll',
      shortcut: 'Ctrl+A',
      category: 'edit'
    });
  }

  canExecute(context: MenuContext): boolean {
    
    return [
      ContextType.EDITOR,
      ContextType.SELECTION
    ].includes(context.type);
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const t = i18nService.getT();
      
      if (context.type === ContextType.EDITOR) {
        return await this.executeForEditor(context as EditorContext);
      }
      
      
      const success = document.execCommand('selectAll');
      
      if (success) {
        return this.success(t('common:contextMenu.status.selectAllSuccess'));
      } else {
        
        const range = document.createRange();
        range.selectNodeContents(context.targetElement);
        const selection = window.getSelection();
        selection?.removeAllRanges();
        selection?.addRange(range);
        
        return this.success(t('common:contextMenu.status.selectAllSuccess'));
      }
    } catch (error) {
      const t = i18nService.getT();
      return this.failure(t('errors:contextMenu.selectAllFailed'), error as Error);
    }
  }
  
   
  private async executeForEditor(context: EditorContext): Promise<CommandResult> {
    try {
      const t = i18nService.getT();
      
      const editor = MonacoHelper.getEditorFromElement(context.targetElement);
      
      if (!editor) {
        log.warn('Editor instance not found, using fallback method');
        
        const success = document.execCommand('selectAll');
        return success
          ? this.success(t('common:contextMenu.status.selectAllSuccess'))
          : this.failure(t('errors:contextMenu.selectAllFailed'));
      }
      
      const model = editor.getModel();
      if (!model) {
        return this.failure(t('errors:contextMenu.editorModelUnavailable'));
      }
      
      
      const lastLine = model.getLineCount();
      const lastColumn = model.getLineMaxColumn(lastLine);
      
      editor.setSelection({
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: lastLine,
        endColumn: lastColumn
      });
      
      
      editor.focus();
      
      return this.success(t('common:contextMenu.status.selectAllSuccess'));
    } catch (error) {
      log.error('Failed to select all in Monaco editor', error as Error);
      const t = i18nService.getT();
      return this.failure(t('errors:contextMenu.selectAllFailed'), error as Error);
    }
  }
}

