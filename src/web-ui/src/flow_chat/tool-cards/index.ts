/**
 * Tool card registry.
 * Maps tool configs to components.
 */

import type { ToolCardConfig } from '../types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import { isMcpToolName, parseMcpToolName } from '@/infrastructure/mcp/toolName';

const log = createLogger('ToolCardRegistry');
// Tool display components
import { ReadFileDisplay } from './ReadFileDisplay';
import { GrepSearchDisplay } from './GrepSearchDisplay';
import { GlobSearchDisplay } from './GlobSearchDisplay';
import { LSDisplay } from './LSDisplay';
import { TodoWriteDisplay } from './TodoWriteDisplay';
import { TaskToolDisplay } from './TaskToolDisplay';
import { CodeReviewToolCard } from './CodeReviewToolCard';
import { FileOperationToolCard } from './FileOperationToolCard';
import { DefaultToolCard } from './DefaultToolCard';
import { WebSearchCard } from './WebSearchCard'; // Temporary until WebSearchDisplay exists.
import { ContextCompressionDisplay } from './ContextCompressionDisplay';
import { MCPToolDisplay } from './MCPToolDisplay';
import { SkillDisplay } from './SkillDisplay';
import { AskUserQuestionCard } from './AskUserQuestionCard';
import { GitToolDisplay } from './GitToolDisplay';
import { GetFileDiffDisplay } from './GetFileDiffDisplay';
import { CreatePlanDisplay } from './CreatePlanDisplay';
import { TerminalToolCard } from './TerminalToolCard';
import { TerminalControlDisplay } from './TerminalControlDisplay';
import { InitMiniAppDisplay } from './MiniAppToolDisplay';
import { GenerativeWidgetToolCard } from './GenerativeWidgetToolCard';
import { ReviewSessionSummaryCard } from './ReviewSessionSummaryCard';
import { SessionControlToolCard } from './SessionControlToolCard';
import { SessionMessageToolCard } from './SessionMessageToolCard';

// Tool card config map - uses backend tool names
export const TOOL_CARD_CONFIGS: Record<string, ToolCardConfig> = {
  // File tools
  'Read': {
    toolName: 'Read',
    displayName: 'Read File',
    icon: 'R',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Read file contents',
    displayMode: 'compact',
    primaryColor: '#3b82f6'
  },
  'Write': {
    toolName: 'Write',
    displayName: 'Write File',
    icon: 'W',
    requiresConfirmation: false, // Snapshot system handles confirmation.
    resultDisplayType: 'summary',
    description: 'Write or create a file',
    displayMode: 'standard',
    primaryColor: '#22c55e'
  },
  'Edit': {
    toolName: 'Edit',
    displayName: 'Edit File',
    icon: 'E',
    requiresConfirmation: false, // Snapshot system handles confirmation.
    resultDisplayType: 'detailed',
    description: 'Edit file contents',
    displayMode: 'standard',
    primaryColor: '#f59e0b'
  },
  'Delete': {
    toolName: 'Delete',
    displayName: 'Delete File',
    icon: 'D',
    requiresConfirmation: false, // Snapshot system handles confirmation.
    resultDisplayType: 'summary',
    description: 'Delete a file',
    displayMode: 'detailed',
    primaryColor: '#ef4444'
  },
  'LS': {
    toolName: 'LS',
    displayName: 'List Directory',
    icon: 'L',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'List directory contents',
    displayMode: 'compact',
    primaryColor: '#6366f1'
  },

  // Search tools
  'Grep': {
    toolName: 'Grep',
    displayName: 'Text Search',
    icon: 'G',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Search text in files',
    displayMode: 'compact',
    primaryColor: '#8b5cf6'
  },
  'Glob': {
    toolName: 'Glob',
    displayName: 'File Search',
    icon: 'F',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Search files by pattern',
    displayMode: 'compact',
    primaryColor: '#06b6d4'
  },

  // Web tools
  'WebSearch': {
    toolName: 'WebSearch',
    displayName: 'Web Search',
    icon: 'WS',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Search the web',
    displayMode: 'compact',
    primaryColor: '#0ea5e9'
  },
  'WebFetch': {
    toolName: 'WebFetch',
    displayName: 'Fetch Link',
    icon: 'WF',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Fetch webpage content',
    displayMode: 'standard',
    primaryColor: '#0ea5e9'
  },

  // Advanced tools
  'Task': {
    toolName: 'Task',
    displayName: 'Run Task',
    icon: '',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Run a specialized AI task',
    displayMode: 'detailed',
    primaryColor: '#7c3aed'
  },
  'TodoWrite': {
    toolName: 'TodoWrite',
    displayName: 'Task Manager',
    icon: 'T',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Manage task lists',
    displayMode: 'standard',
    primaryColor: '#0d9488'
  },
  'submit_code_review': {
    toolName: 'submit_code_review',
    displayName: 'Code Review',
    icon: 'CR',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Submit code review results',
    displayMode: 'compact',
    primaryColor: '#8b5cf6'
  },
  'ContextCompression': {
    toolName: 'ContextCompression',
    displayName: 'Context Compression',
    icon: 'CC',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Compress conversation context to reduce tokens',
    displayMode: 'compact',
    primaryColor: '#a855f7'
  },

  // Skill tool
  'Skill': {
    toolName: 'Skill',
    displayName: 'Skill',
    icon: 'S',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Load and run skills',
    displayMode: 'compact',
    primaryColor: '#8b5cf6'
  },

  // AskUserQuestion tool
  'AskUserQuestion': {
    toolName: 'AskUserQuestion',
    displayName: 'Ask User',
    icon: 'Q',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Ask the user a question and wait for a reply',
    displayMode: 'detailed',
    primaryColor: '#8b5cf6'
  },

  'ReviewSessionSummary': {
    toolName: 'ReviewSessionSummary',
    displayName: 'Review summary',
    icon: 'REV',
    requiresConfirmation: false,
    resultDisplayType: 'hidden',
    description: 'Review session summary marker',
    displayMode: 'detailed',
    primaryColor: '#0ea5e9'
  },

  // Git version control tool
  'Git': {
    toolName: 'Git',
    displayName: 'Git',
    icon: 'GIT',
    requiresConfirmation: false, // Read-only needs no confirmation; writes are backend-controlled.
    resultDisplayType: 'detailed',
    description: 'Run Git commands',
    displayMode: 'compact',
    primaryColor: '#f97316' // Orange, Git brand color
  },

  // GetFileDiff tool
  'GetFileDiff': {
    toolName: 'GetFileDiff',
    displayName: 'File Diff',
    icon: 'DIFF',
    requiresConfirmation: false, // Read-only tool.
    resultDisplayType: 'detailed',
    description: 'Get file diffs (Baseline/Git/Full)',
    displayMode: 'compact',
    primaryColor: '#8b5cf6' // Purple
  },

  // CreatePlan tool
  'CreatePlan': {
    toolName: 'CreatePlan',
    displayName: 'Create Plan',
    icon: 'PLAN',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Create and manage project plans',
    displayMode: 'detailed',
    primaryColor: '#f59e0b' // Orange
  },

  // TerminalControl tool
  'TerminalControl': {
    toolName: 'TerminalControl',
    displayName: 'Terminal Control',
    icon: 'TC',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Kill or interrupt a terminal session',
    displayMode: 'compact',
    primaryColor: '#ef4444'
  },

  'SessionControl': {
    toolName: 'SessionControl',
    displayName: 'Session Control',
    icon: 'SC',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Create, delete, or list sessions',
    displayMode: 'compact',
    primaryColor: '#3b82f6'
  },

  'SessionMessage': {
    toolName: 'SessionMessage',
    displayName: 'Session Message',
    icon: 'SM',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Send a message to another session',
    displayMode: 'compact',
    primaryColor: '#8b5cf6'
  },

  // Bash terminal tool
  'Bash': {
    toolName: 'Bash',
    displayName: 'Run Command',
    icon: 'TERM',
    requiresConfirmation: true, // Requires user confirmation.
    resultDisplayType: 'detailed',
    description: 'Run commands in the terminal',
    displayMode: 'standard',
    primaryColor: '#10b981' // Teal, classic terminal color
  },

  // MiniApp tool
  'InitMiniApp': {
    toolName: 'InitMiniApp',
    displayName: 'Init Mini App',
    icon: 'APP',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Create Mini App skeleton for editing',
    displayMode: 'standard',
    primaryColor: '#7c8cef'
  },
  'GenerativeUI': {
    toolName: 'GenerativeUI',
    displayName: 'Generative UI',
    icon: 'UI',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Render interactive widget previews inline in FlowChat',
    displayMode: 'detailed',
    primaryColor: '#38bdf8'
  },
};

// Tool card component map - uses backend tool names
export const TOOL_CARD_COMPONENTS = {
  // File tools
  'Read': ReadFileDisplay, // Read does not need snapshot support.
  'Write': FileOperationToolCard,
  'Edit': FileOperationToolCard,
  'Delete': FileOperationToolCard,
  
  // Search tools
  'Grep': GrepSearchDisplay,
  'Glob': GlobSearchDisplay,
  'LS': LSDisplay,
  
  // Web tools
  'WebSearch': WebSearchCard,
  
  // Advanced tools
  'Task': TaskToolDisplay,
  'TodoWrite': TodoWriteDisplay,
  
  'submit_code_review': CodeReviewToolCard,
  
  // Context compression
  'ContextCompression': ContextCompressionDisplay,

  // Skill tool
  'Skill': SkillDisplay,

  // AskUserQuestion tool
  'AskUserQuestion': AskUserQuestionCard,

  'ReviewSessionSummary': ReviewSessionSummaryCard,

  // Git version control
  'Git': GitToolDisplay,

  // GetFileDiff tool
  'GetFileDiff': GetFileDiffDisplay,

  // CreatePlan tool
  'CreatePlan': CreatePlanDisplay,

  // TerminalControl tool
  'TerminalControl': TerminalControlDisplay,

  // Session tools
  'SessionControl': SessionControlToolCard,
  'SessionMessage': SessionMessageToolCard,

  // Bash tool
  'Bash': TerminalToolCard,

  // MiniApp tool
  'InitMiniApp': InitMiniAppDisplay,

  // Generative widget tool
  'GenerativeUI': GenerativeWidgetToolCard,
};

/**
 * Get tool card config.
 */
export function getToolCardConfig(toolName: string): ToolCardConfig {
  // Check MCP tools (prefix: mcp__).
  if (isMcpToolName(toolName)) {
    const parsed = parseMcpToolName(toolName);
    const actualToolName = parsed?.toolName ?? toolName;

    return {
      toolName,
      displayName: actualToolName || toolName,
      icon: 'MCP',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
      description: 'MCP',
      displayMode: 'compact',
      primaryColor: '#8b5cf6'
    };
  }
  
  // Match by name or fall back to defaults.
  return TOOL_CARD_CONFIGS[toolName] || {
    toolName,
    displayName: `Tool: ${toolName}`,
    icon: 'TOOL',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: `Run ${toolName} tool`,
    displayMode: 'standard',
    primaryColor: '#6b7280'
  };
}

/**
 * Get tool card component.
 */
export function getToolCardComponent(toolName: string) {
  // Check MCP tools (prefix: mcp__).
  if (isMcpToolName(toolName)) {
    return MCPToolDisplay;
  }
  
  const component = TOOL_CARD_COMPONENTS[toolName as keyof typeof TOOL_CARD_COMPONENTS];
  
  // Debug log (only when a component is missing).
  if (!component) {
    log.warn('Tool card component not found, using default', { toolName });
  }
  
  return component || DefaultToolCard;
}

/**
 * Check whether a tool needs confirmation.
 */
export function requiresConfirmation(toolName: string): boolean {
  const config = getToolCardConfig(toolName);
  return config.requiresConfirmation;
}

/**
 * Get all registered tool names.
 */
export function getAllToolNames(): string[] {
  return Object.keys(TOOL_CARD_CONFIGS);
}

// Export components
export {
  BaseToolCard,
  ToolCardHeader,
} from './BaseToolCard';
export {
  ToolCardHeaderLayoutContext,
  useToolCardHeaderLayout,
} from './ToolCardHeaderLayoutContext';
export type {
  BaseToolCardProps,
  ToolCardHeaderProps,
} from './BaseToolCard';
export type {
  ToolCardHeaderLayoutContextValue,
  ToolCardHeaderAffordanceKind,
} from './ToolCardHeaderLayoutContext';
export { ToolCardIconSlot } from './ToolCardIconSlot';
export type { ToolCardIconSlotProps } from './ToolCardIconSlot';
export { ToolCardStatusIcon } from './ToolCardStatusIcon';
export type { ToolCardStatusIconProps } from './ToolCardStatusIcon';
export { PlanDisplay } from './CreatePlanDisplay';
export type { PlanDisplayProps } from './CreatePlanDisplay';

// ==================== Collapsible explorer tools ====================

import type { FlowItem, FlowToolItem } from '../types/flow-chat';

/**
 * Collapsible explorer tools.
 * They are auto-collapsed during streaming to reduce visual noise.
 */
export const COLLAPSIBLE_TOOL_NAMES = new Set([
  'Read', 'LS', 'Grep', 'Glob', 'WebSearch', 'Bash', 'Git',
]);

/** Read tools (counted in readCount). */
export const READ_TOOL_NAMES = new Set(['Read', 'LS']);

/** Search tools (counted in searchCount). */
export const SEARCH_TOOL_NAMES = new Set(['Grep', 'Glob', 'WebSearch']);

/** Command tools (counted in commandCount). */
export const COMMAND_TOOL_NAMES = new Set(['Bash', 'Git']);

/** Check whether a tool is collapsible. */
export function isCollapsibleTool(toolName: string): boolean {
  return COLLAPSIBLE_TOOL_NAMES.has(toolName);
}

/**
 * Check whether a FlowItem is collapsible (no context).
 * - Subagent items are never collapsed.
 * - Text needs context (use isCollapsibleItemWithContext).
 * - Thinking can be collapsed with explorer tools.
 * - Only explorer tools are collapsible.
 */
export function isCollapsibleItem(item: FlowItem): boolean {
  // Subagent items are never collapsed.
  if ((item as any).isSubagentItem) return false;
  
  // Text: default not collapsed (needs isCollapsibleItemWithContext).
  if (item.type === 'text') return false;
  
  // Thinking can be collapsed with explorer tools.
  if (item.type === 'thinking') return true;
  
  // Tools: only explorer tools are collapsible.
  if (item.type === 'tool') {
    return isCollapsibleTool((item as FlowToolItem).toolName);
  }
  
  return false;
}

/**
 * Check whether a FlowItem is collapsible with context.
 * @param item Current item
 * @param nextItem Next item (optional)
 * @param isLast Whether this is the last item
 */
export function isCollapsibleItemWithContext(
  item: FlowItem, 
  nextItem: FlowItem | undefined, 
  isLast: boolean
): boolean {
  // Subagent items are never collapsed.
  if ((item as any).isSubagentItem) return false;
  
  // Text and thinking depend on what follows.
  if (item.type === 'text' || item.type === 'thinking') {
    // Last item should stay visible.
    if (isLast || !nextItem) return false;
    
    // If followed by an explorer tool, collapse together.
    if (nextItem.type === 'tool') {
      return isCollapsibleTool((nextItem as FlowToolItem).toolName);
    }
    
    // If followed by text or thinking, treat as collapsible for grouping.
    if (nextItem.type === 'text' || nextItem.type === 'thinking') {
      return true;
    }
    
    // Otherwise do not collapse.
    return false;
  }
  
  // Tools: only explorer tools are collapsible.
  if (item.type === 'tool') {
    return isCollapsibleTool((item as FlowToolItem).toolName);
  }
  
  return false;
}
