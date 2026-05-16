/**
 * FlowChat Cards Component Library
 * Specialized components for displaying tool execution processes and results in FlowChat
 */

import { i18nService } from '@/infrastructure/i18n';

export { BaseToolCard } from './BaseToolCard';
export type { BaseToolCardProps } from './BaseToolCard';

export { ToolProcessingDots } from './ToolProcessingDots';
export type { ToolProcessingDotsProps, ToolProcessingDotsSize } from './ToolProcessingDots';

export { SnapshotCard } from './SnapshotCard';
export type { SnapshotCardProps } from './SnapshotCard';

export { SearchCard } from './SearchCard';
export type { SearchCardProps } from './SearchCard';

export { TaskCard } from './TaskCard';
export type { TaskCardProps } from './TaskCard';

export { TodoCard } from './TodoCard';
export type { TodoCardProps, TodoItem } from './TodoCard';

export { WebSearchCard } from './WebSearchCard';
export type { WebSearchCardProps, WebSearchResult } from './WebSearchCard';

export { ContextCompressionCard } from './ContextCompressionCard';
export type { ContextCompressionCardProps } from './ContextCompressionCard';

export interface ToolCardConfig {
  toolName: string;
  displayName: string;
  icon: string;
  requiresConfirmation: boolean;
  resultDisplayType: 'summary' | 'detailed' | 'hidden';
  description: string;
  displayMode: 'compact' | 'standard' | 'detailed' | 'terminal';
  primaryColor: string;
}

export const FLOWCHAT_CARD_CONFIGS: Record<string, ToolCardConfig> = {
  'Read': {
    toolName: 'Read',
    displayName: i18nService.t('components:flowChatCards.toolConfig.read.displayName'),
    icon: 'R',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: i18nService.t('components:flowChatCards.toolConfig.read.description'),
    displayMode: 'compact',
    primaryColor: '#3b82f6'
  },
  'Write': {
    toolName: 'Write',
    displayName: i18nService.t('components:flowChatCards.toolConfig.write.displayName'),
    icon: 'W',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: i18nService.t('components:flowChatCards.toolConfig.write.description'),
    displayMode: 'standard',
    primaryColor: '#22c55e'
  },
  'Edit': {
    toolName: 'Edit',
    displayName: i18nService.t('components:flowChatCards.toolConfig.edit.displayName'),
    icon: 'E',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: i18nService.t('components:flowChatCards.toolConfig.edit.description'),
    displayMode: 'standard',
    primaryColor: '#f59e0b'
  },
  'Delete': {
    toolName: 'Delete',
    displayName: i18nService.t('components:flowChatCards.toolConfig.delete.displayName'),
    icon: 'D',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: i18nService.t('components:flowChatCards.toolConfig.delete.description'),
    displayMode: 'detailed',
    primaryColor: '#ef4444'
  },

  'Grep': {
    toolName: 'Grep',
    displayName: i18nService.t('components:flowChatCards.toolConfig.grep.displayName'),
    icon: 'G',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: i18nService.t('components:flowChatCards.toolConfig.grep.description'),
    displayMode: 'compact',
    primaryColor: '#8b5cf6'
  },
  'Glob': {
    toolName: 'Glob',
    displayName: i18nService.t('components:flowChatCards.toolConfig.glob.displayName'),
    icon: 'F',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: i18nService.t('components:flowChatCards.toolConfig.glob.description'),
    displayMode: 'compact',
    primaryColor: '#06b6d4'
  },

  'WebSearch': {
    toolName: 'WebSearch',
    displayName: i18nService.t('components:flowChatCards.toolConfig.webSearch.displayName'),
    icon: 'WS',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: i18nService.t('components:flowChatCards.toolConfig.webSearch.description'),
    displayMode: 'compact',
    primaryColor: '#0ea5e9'
  },
  'WebFetch': {
    toolName: 'WebFetch',
    displayName: i18nService.t('components:flowChatCards.toolConfig.webFetch.displayName'),
    icon: 'WF',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: i18nService.t('components:flowChatCards.toolConfig.webFetch.description'),
    displayMode: 'standard',
    primaryColor: '#0ea5e9'
  },

  'Task': {
    toolName: 'Task',
    displayName: i18nService.t('components:flowChatCards.toolConfig.task.displayName'),
    icon: 'AI',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: i18nService.t('components:flowChatCards.toolConfig.task.description'),
    displayMode: 'detailed',
    primaryColor: '#7c3aed'
  },
  'TodoWrite': {
    toolName: 'TodoWrite',
    displayName: i18nService.t('components:flowChatCards.toolConfig.todoWrite.displayName'),
    icon: 'T',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: i18nService.t('components:flowChatCards.toolConfig.todoWrite.description'),
    displayMode: 'standard',
    primaryColor: '#0d9488'
  },
  'ContextCompression': {
    toolName: 'ContextCompression',
    displayName: i18nService.t('components:flowChatCards.toolConfig.contextCompression.displayName'),
    icon: 'CC',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: i18nService.t('components:flowChatCards.toolConfig.contextCompression.description'),
    displayMode: 'standard',
    primaryColor: '#a855f7'
  }
};

export function getFlowChatCardConfig(toolName: string): ToolCardConfig {
  return FLOWCHAT_CARD_CONFIGS[toolName] || {
    toolName,
    displayName: toolName,
    icon: '•',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: i18nService.t('components:flowChatCards.toolConfig.default.description', { toolName }),
    displayMode: 'standard',
    primaryColor: '#6b7280'
  };
}

export function requiresConfirmation(toolName: string): boolean {
  const config = getFlowChatCardConfig(toolName);
  return config.requiresConfirmation;
}

export function getAllFlowChatCardToolNames(): string[] {
  return Object.keys(FLOWCHAT_CARD_CONFIGS);
}
