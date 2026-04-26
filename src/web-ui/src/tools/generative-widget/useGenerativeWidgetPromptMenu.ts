import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { globalEventBus } from '@/infrastructure/event-bus';
import { useContextMenuStore } from '@/shared/context-menu-system';
import { ContextType } from '@/shared/context-menu-system/types/context.types';
import type { MenuItem } from '@/shared/context-menu-system/types/menu.types';
import { notificationService } from '@/shared/notification-system';
import type { WidgetContextMenuMessage } from './GenerativeWidgetFrame';
import { createWidgetPromptReferenceToken } from './widgetPromptReference';

function buildWidgetPromptReference(
  event: WidgetContextMenuMessage,
  t: (key: string, options?: Record<string, unknown>) => string,
): { promptText: string; displayText: string } {
  const lines: string[] = [];
  const summary = event.elementSummary?.trim() || t('widgetContextMenu.unknownElement');
  const section = event.sectionSummary?.trim();
  const filePath = event.filePath?.trim();
  const line =
    typeof event.line === 'number' && Number.isFinite(event.line) && event.line > 0
      ? `:${event.line}`
      : '';

  lines.push(t('widgetContextMenu.promptSelected', { summary }));

  if (section) {
    lines.push(t('widgetContextMenu.promptSection', { section }));
  }

  if (filePath) {
    lines.push(t('widgetContextMenu.promptFile', { file: filePath, line }));
  }

  return {
    promptText: lines.join('\n'),
    displayText: t('widgetContextMenu.displayText', { summary }),
  };
}

export function useGenerativeWidgetPromptMenu(source: 'tool-card' | 'panel') {
  const { t } = useTranslation('flow-chat');
  const showMenu = useContextMenuStore(state => state.showMenu);

  return useCallback((
    event: WidgetContextMenuMessage,
    targetElement?: HTMLElement | null,
  ) => {
    const x = Number(event.viewportX);
    const y = Number(event.viewportY);

    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      return;
    }

    const promptReference = buildWidgetPromptReference(event, t);
    const promptToken = createWidgetPromptReferenceToken({
      promptText: promptReference.promptText,
      displayText: promptReference.displayText,
      summary: event.elementSummary?.trim() || t('widgetContextMenu.unknownElement'),
      sectionSummary: event.sectionSummary?.trim(),
      filePath: event.filePath?.trim(),
      line: event.line,
    });
    const items: MenuItem[] = [
      {
        id: `widget-add-to-input:${event.widgetId || 'unknown'}`,
        label: t('widgetContextMenu.addToInput'),
        icon: 'MessageSquarePlus',
        onClick: () => {
          globalEventBus.emit(
            'fill-chat-input',
            {
              content: promptToken,
              mode: 'append',
            },
            'generativeWidgetPromptMenu',
          );
          notificationService.success(t('widgetContextMenu.addedToInput'), {
            duration: 2000,
          });
        },
      },
    ];

    showMenu(
      { x, y },
      items,
      {
        type: ContextType.CUSTOM,
        customType: 'generative-widget-element',
        data: {
          source,
          widgetId: event.widgetId,
          promptText: promptReference.promptText,
          promptToken,
          event,
        },
        event: new MouseEvent('contextmenu', {
          bubbles: true,
          cancelable: true,
          clientX: x,
          clientY: y,
        }),
        targetElement: targetElement || document.body,
        position: { x, y },
        timestamp: Date.now(),
      },
    );
  }, [showMenu, source, t]);
}

export default useGenerativeWidgetPromptMenu;
