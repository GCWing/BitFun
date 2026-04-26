 

import { contextMenuRegistry } from '@/shared/context-menu-system';
import type { MenuContext, MenuItem } from '@/shared/context-menu-system/types';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('NotificationContextMenuProvider');

 
export function registerNotificationContextMenu(): void {
  contextMenuRegistry.register({
    id: 'notification-menu',
    name: i18nService.t('common:contextMenu.notificationMenu.name'),
    description: i18nService.t('common:contextMenu.notificationMenu.description'),
    priority: 100,
    matcher: (context: MenuContext) => {
      
      const target = context.targetElement;
      return !!(target?.closest('[data-context-type="notification"]'));
    },
    menuBuilder: (context: MenuContext): MenuItem[] => {
      
      const notificationElement = context.targetElement?.closest('[data-context-type="notification"]') as HTMLElement;
      if (!notificationElement) {
        return [];
      }

      const title = notificationElement.getAttribute('data-notification-title') || '';
      const message = notificationElement.getAttribute('data-notification-message') || '';
      const diagnostics = notificationElement.getAttribute('data-notification-diagnostics') || '';

      const items: MenuItem[] = [
        {
          id: 'copy-title',
          label: i18nService.t('common:contextMenu.notificationMenu.items.copyTitle'),
          icon: 'Copy',
          onClick: () => {
            navigator.clipboard.writeText(title);
          }
        },
        {
          id: 'copy-message',
          label: i18nService.t('common:contextMenu.notificationMenu.items.copyMessage'),
          icon: 'Copy',
          onClick: () => {
            navigator.clipboard.writeText(message);
          }
        },
      ];

      if (diagnostics) {
        items.push({
          id: 'copy-diagnostics',
          label: i18nService.t('errors:ai.actions.copyDiagnostics'),
          icon: 'Copy',
          onClick: () => {
            navigator.clipboard.writeText(diagnostics);
          }
        });
      }

      items.push(
        {
          id: 'divider-1',
          label: '',
          separator: true
        },
        {
          id: 'copy-all',
          label: i18nService.t('common:contextMenu.notificationMenu.items.copyAll'),
          icon: 'Copy',
          onClick: () => {
            const text = [title, message, diagnostics].filter(Boolean).join('\n');
            navigator.clipboard.writeText(text);
          }
        }
      );

      return items;
    }
  });

  log.info('Notification context menu provider registered');
}

