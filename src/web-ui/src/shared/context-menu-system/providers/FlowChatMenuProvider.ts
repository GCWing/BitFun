 

import { IMenuProvider } from '../types/provider.types';
import { MenuItem } from '../types/menu.types';
import { MenuContext, ContextType, FlowChatContext } from '../types/context.types';
import { commandExecutor } from '../commands/CommandExecutor';
import { globalEventBus } from '../../../infrastructure/event-bus';
import { i18nService } from '../../../infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('FlowChatMenuProvider');

export class FlowChatMenuProvider implements IMenuProvider {
  readonly id = 'flowchat';
  readonly name = i18nService.t('common:contextMenu.flowChatMenu.name');
  readonly description = i18nService.t('common:contextMenu.flowChatMenu.description');
  readonly priority = 70;

  matches(context: MenuContext): boolean {
    return context.type === ContextType.FLOWCHAT ||
           context.type === ContextType.FLOWCHAT_TOOL_CARD ||
           context.type === ContextType.FLOWCHAT_TEXT_BLOCK;
  }

  async getMenuItems(context: MenuContext): Promise<MenuItem[]> {
    const flowChatContext = context as FlowChatContext;
    const items: MenuItem[] = [];
    
    const { selectedText, dialogTurn, metadata } = flowChatContext;

    
    if (selectedText && selectedText.trim()) {
      items.push({
        id: 'flowchat-copy-selected',
        label: i18nService.t('flow-chat:contextMenu.copySelection'),
        icon: 'Copy',
        shortcut: 'Ctrl+C',
        command: 'copy',
        onClick: async (ctx) => {
          await commandExecutor.execute('copy', ctx);
          globalEventBus.emit('flowchat:text-copied', { 
            text: selectedText, 
            type: 'selection' 
          });
        }
      });
    }

    
    const elementText = this.getElementText(flowChatContext.targetElement);
    if (elementText && elementText.trim() && elementText !== selectedText) {
      items.push({
        id: 'flowchat-copy-element',
        label: selectedText
          ? i18nService.t('flow-chat:contextMenu.copyFullContent')
          : i18nService.t('flow-chat:contextMenu.copyContent'),
        icon: 'Copy',
        shortcut: selectedText ? undefined : 'Ctrl+C',
        onClick: async () => {
          try {
            await navigator.clipboard.writeText(elementText);
            globalEventBus.emit('flowchat:text-copied', { 
              text: elementText, 
              type: 'element' 
            });
          } catch (error) {
            log.error('Failed to copy element text', error as Error);
            globalEventBus.emit('toast:error', { message: i18nService.t('errors:general.copyFailed') });
          }
        }
      });
    }

    
    if (dialogTurn && items.length > 0) {
      items.push({
        id: 'flowchat-separator-1',
        label: '',
        separator: true
      });
      
      items.push({
        id: 'flowchat-copy-dialog',
        label: i18nService.t('flow-chat:contextMenu.copyDialog'),
        icon: 'MessageSquare',
        onClick: () => {
          globalEventBus.emit('flowchat:copy-dialog', { 
            dialogTurn, 
            context: flowChatContext 
          });
        }
      });
    }

    
    if (context.type === ContextType.FLOWCHAT_TOOL_CARD && metadata?.flowItem) {
      const flowItem = metadata.flowItem;
      
      if (items.length > 0) {
        items.push({
          id: 'flowchat-separator-2',
          label: '',
          separator: true
        });
      }

      items.push({
        id: 'flowchat-copy-tool-input',
        label: i18nService.t('flow-chat:contextMenu.copyToolInput'),
        icon: 'FileInput',
        onClick: async () => {
          try {
            const input = JSON.stringify(flowItem.toolCall?.input || {}, null, 2);
            await navigator.clipboard.writeText(input);
            globalEventBus.emit('flowchat:tool-data-copied', { 
              type: 'input', 
              data: input 
            });
          } catch (error) {
            log.error('Failed to copy tool input', error as Error);
            globalEventBus.emit('toast:error', { message: i18nService.t('errors:general.copyFailed') });
          }
        }
      });

      if (flowItem.toolResult) {
        items.push({
          id: 'flowchat-copy-tool-output',
          label: i18nService.t('flow-chat:contextMenu.copyToolOutput'),
          icon: 'FileOutput',
          onClick: async () => {
            try {
              const output = typeof flowItem.toolResult.result === 'string' 
                ? flowItem.toolResult.result 
                : JSON.stringify(flowItem.toolResult.result, null, 2);
              await navigator.clipboard.writeText(output);
              globalEventBus.emit('flowchat:tool-data-copied', { 
                type: 'output', 
                data: output 
              });
            } catch (error) {
              log.error('Failed to copy tool output', error as Error);
              globalEventBus.emit('toast:error', { message: i18nService.t('errors:general.copyFailed') });
            }
          }
        });
      }
    }

    return items;
  }

  isEnabled(): boolean {
    return true;
  }

   
  private getElementText(element: HTMLElement): string {
    if (!element) return '';
    
    
    const text = element.textContent || element.innerText || '';
    return text.trim();
  }
}

