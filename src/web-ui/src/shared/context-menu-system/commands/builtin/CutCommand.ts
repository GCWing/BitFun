 

import { BaseCommand } from '../BaseCommand';
import { CommandResult } from '../../types/command.types';
import { MenuContext, ContextType, SelectionContext, EditorContext } from '../../types/context.types';
import { MonacoHelper } from '@/shared/helpers/MonacoHelper';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('CutCommand');

export class CutCommand extends BaseCommand {
  constructor() {
    const t = i18nService.getT();
    super({
      id: 'cut',
      label: t('common:actions.cut'),
      description: t('common:contextMenu.descriptions.cut'),
      icon: 'Scissors',
      shortcut: 'Ctrl+X',
      category: 'edit'
    });
  }

  canExecute(context: MenuContext): boolean {
    
    if (context.type === ContextType.SELECTION) {
      const selectionContext = context as SelectionContext;
      return !!selectionContext.selectedText && selectionContext.isEditable;
    }
    
    if (context.type === ContextType.EDITOR) {
      const editorContext = context as EditorContext;
      return !!editorContext.selectedText && !editorContext.isReadOnly;
    }
    
    return false;
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const t = i18nService.getT();
      
      if (context.type === ContextType.EDITOR) {
        return await this.executeForEditor(context as EditorContext);
      }
      
      
      const selectionContext = context as SelectionContext;
      const text = selectionContext.selectedText;

      if (!text) {
        return this.failure(t('errors:contextMenu.noSelection'));
      }

      if (!selectionContext.isEditable) {
        return this.failure(t('errors:contextMenu.notEditable'));
      }

      
      if (navigator.clipboard) {
        await navigator.clipboard.writeText(text);
      } else {
        const textarea = document.createElement('textarea');
        textarea.value = text;
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
      }

      
      document.execCommand('delete');

      return this.success(t('common:contextMenu.status.cutSuccess'), { text });
    } catch (error) {
      const t = i18nService.getT();
      return this.failure(t('errors:contextMenu.cutFailed'), error as Error);
    }
  }
  
   
  private async executeForEditor(context: EditorContext): Promise<CommandResult> {
    try {
      const t = i18nService.getT();
      const text = context.selectedText;
      
      if (!text) {
        return this.failure(t('errors:contextMenu.noSelection'));
      }
      
      if (context.isReadOnly) {
        return this.failure(t('errors:contextMenu.readOnlyEditor'));
      }
      
      
      const editor = MonacoHelper.getEditorFromElement(context.targetElement);
      
      if (!editor) {
        log.warn('Editor instance not found, using fallback method');
        
        if (navigator.clipboard) {
          await navigator.clipboard.writeText(text);
        }
        document.execCommand('delete');
        return this.success(t('common:contextMenu.status.cutSuccess'), { text });
      }
      
      
      if (navigator.clipboard) {
        await navigator.clipboard.writeText(text);
      } else {
        const textarea = document.createElement('textarea');
        textarea.value = text;
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
      }
      
      
      const selection = editor.getSelection();
      if (selection) {
        editor.executeEdits('cut', [{
          range: selection,
          text: ''
        }]);
      }
      
      editor.focus();
      
      return this.success(t('common:contextMenu.status.cutSuccess'), { text });
    } catch (error) {
      log.error('Failed to cut in Monaco editor', error as Error);
      const t = i18nService.getT();
      return this.failure(t('errors:contextMenu.cutFailed'), error as Error);
    }
  }
}

