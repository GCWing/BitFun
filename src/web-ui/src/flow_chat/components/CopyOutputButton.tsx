/**
 * Button to copy a dialog turn output.
 * Copies all AI text and tool calls from the turn.
 */

import React, { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, Check, Edit } from 'lucide-react';
import type { DialogTurn, FlowTextItem, FlowToolItem } from '../types/flow-chat';
import { createMarkdownEditorTab } from '@/shared/utils/tabUtils';
import { Tooltip } from '@/component-library';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import './CopyOutputButton.css';

const log = createLogger('CopyOutputButton');

interface CopyOutputButtonProps {
  dialogTurn: DialogTurn;
  className?: string;
}

export const CopyOutputButton: React.FC<CopyOutputButtonProps> = ({
  dialogTurn,
  className = ''
}) => {
  const { t } = useTranslation('flow-chat');
  const [copied, setCopied] = useState(false);

  const extractOutputContent = useCallback((dialogTurn: DialogTurn): string => {
    const contentParts: string[] = [];

    dialogTurn.modelRounds.forEach((modelRound) => {
      const sortedItems = [...modelRound.items].sort((a, b) => a.timestamp - b.timestamp);

      sortedItems.forEach((item) => {
        if (item.type === 'text') {
          const textItem = item as FlowTextItem;
          if (textItem.content.trim()) {
            contentParts.push(textItem.content.trim());
          }
        } else if (item.type === 'tool') {
          const toolItem = item as FlowToolItem;
          
          if (toolItem.toolCall) {
            const toolName = toolItem.toolName || t('copyOutput.unknownTool');
            contentParts.push(t('copyOutput.toolCall', { name: toolName }));
          }
        }
      });
    });

    return contentParts.join('\n\n');
  }, [t]);

  const handleCopy = useCallback(async () => {
    try {
      const content = extractOutputContent(dialogTurn);
      if (!content.trim()) {
        log.warn('No content to copy');
        return;
      }

      await navigator.clipboard.writeText(content);
      setCopied(true);
      
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      log.error('Failed to copy', error);
    }
  }, [dialogTurn, extractOutputContent]);

  const handleOpenInEditor = useCallback(() => {
    try {
      const content = extractOutputContent(dialogTurn);
      if (!content.trim()) {
        log.warn('No content to edit');
        return;
      }

      window.dispatchEvent(new CustomEvent('expand-right-panel'));

      setTimeout(() => {
        const timestamp = i18nService.formatDate(new Date(), { 
          month: '2-digit', 
          day: '2-digit', 
          hour: '2-digit', 
          minute: '2-digit' 
        }).replace(/\//g, '-');
        
        createMarkdownEditorTab(
          t('copyOutput.aiReply', { timestamp }),
          content,
          undefined, // No filePath: create a temporary editor.
          undefined,
          'agent'
        );
        
        log.debug('AI reply opened in editor');
      }, 250);
    } catch (error) {
      log.error('Failed to open editor', error);
    }
  }, [dialogTurn, extractOutputContent, t]);

  const hasContent = dialogTurn.modelRounds.some(round => 
    round.items.some(item => 
      (item.type === 'text' && (item as FlowTextItem).content.trim()) ||
      (item.type === 'tool' && (item as FlowToolItem).toolCall)
    )
  );

  if (!hasContent) {
    return null;
  }

  return (
    <div className={`copy-output-button-group ${className}`}>
      <button
        className={`copy-output-button ${copied ? 'copied' : ''}`}
        onClick={handleCopy}
        title={copied ? t('copyOutput.copiedOutputContent') : t('copyOutput.copyOutputContent')}
        aria-label={copied ? t('copyOutput.copiedOutputContent') : t('copyOutput.copyOutputContent')}
      >
        <span className="button-icon">
          {copied ? <Check size={14} /> : <Copy size={14} />}
        </span>
        <span className="button-text">
          {copied ? t('copyOutput.copied') : t('copyOutput.copy')}
        </span>
      </button>
      
      <Tooltip content={t('copyOutput.openInEditor')}>
        <button
          className="copy-output-button edit-button"
          onClick={handleOpenInEditor}
          aria-label={t('copyOutput.openInEditor')}
        >
          <span className="button-icon">
            <Edit size={14} />
          </span>
          <span className="button-text">
            {t('copyOutput.edit')}
          </span>
        </button>
      </Tooltip>
    </div>
  );
};
