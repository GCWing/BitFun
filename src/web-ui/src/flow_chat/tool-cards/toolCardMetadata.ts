/**
 * Tool-card metadata and lightweight helpers.
 *
 * Keep this module free of card component imports so startup-visible callers can
 * inspect tool behavior without pulling heavy renderers into the first bundle.
 */

import type { FlowItem, FlowToolItem, ToolCardConfig } from '../types/flow-chat';
import { isMcpToolName, parseMcpToolName } from '@/infrastructure/mcp/toolName';

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
    primaryColor: 'var(--color-accent-600)'
  },
  'Write': {
    toolName: 'Write',
    displayName: 'Write File',
    icon: 'W',
    requiresConfirmation: false, // Snapshot system handles confirmation.
    resultDisplayType: 'summary',
    description: 'Write or create a file',
    displayMode: 'standard',
    primaryColor: 'var(--tool-card-write-color)'
  },
  'Edit': {
    toolName: 'Edit',
    displayName: 'Edit File',
    icon: 'E',
    requiresConfirmation: false, // Snapshot system handles confirmation.
    resultDisplayType: 'detailed',
    description: 'Edit file contents',
    displayMode: 'standard',
    primaryColor: 'var(--color-warning)'
  },
  'Delete': {
    toolName: 'Delete',
    displayName: 'Delete File',
    icon: 'D',
    requiresConfirmation: false, // Snapshot system handles confirmation.
    resultDisplayType: 'summary',
    description: 'Delete a file',
    displayMode: 'detailed',
    primaryColor: 'var(--color-error)'
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
    primaryColor: 'var(--color-purple-500)'
  },
  'Glob': {
    toolName: 'Glob',
    displayName: 'File Search',
    icon: 'F',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Search files by pattern',
    displayMode: 'compact',
    primaryColor: 'var(--tool-card-file-search-color)'
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
    primaryColor: 'var(--tool-card-web-search-color)'
  },
  'WebFetch': {
    toolName: 'WebFetch',
    displayName: 'Read Webpage',
    icon: 'WF',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Fetch webpage content',
    displayMode: 'standard',
    primaryColor: 'var(--tool-card-web-search-color)'
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
    primaryColor: 'var(--color-purple-600)'
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
    primaryColor: 'var(--color-purple-500)'
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
  'GetToolSpec': {
    toolName: 'GetToolSpec',
    displayName: 'Read Tool Spec',
    icon: 'SPEC',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Read usage instructions and schema for a collapsed tool',
    displayMode: 'compact',
    primaryColor: '#14b8a6'
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
    primaryColor: 'var(--color-purple-500)'
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
    primaryColor: 'var(--color-purple-500)'
  },

  'ReviewSessionSummary': {
    toolName: 'ReviewSessionSummary',
    displayName: 'Review summary',
    icon: 'REV',
    requiresConfirmation: false,
    resultDisplayType: 'hidden',
    description: 'Review session summary marker',
    displayMode: 'detailed',
    primaryColor: 'var(--tool-card-web-search-color)'
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
    primaryColor: 'var(--tool-card-git-color)' // Orange, Git brand color
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
    primaryColor: 'var(--color-purple-500)'
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
    primaryColor: 'var(--color-warning)'
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
    primaryColor: 'var(--color-error)'
  },

  'SessionControl': {
    toolName: 'SessionControl',
    displayName: 'Session Control',
    icon: 'SC',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Create, delete, or list sessions',
    displayMode: 'compact',
    primaryColor: 'var(--color-accent-600)'
  },

  'SessionMessage': {
    toolName: 'SessionMessage',
    displayName: 'Session Message',
    icon: 'SM',
    requiresConfirmation: false,
    resultDisplayType: 'summary',
    description: 'Send a message to another session',
    displayMode: 'compact',
    primaryColor: 'var(--color-purple-500)'
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
    primaryColor: 'var(--tool-card-terminal-color)' // Teal, classic terminal color
  },

  'ExecCommand': {
    toolName: 'ExecCommand',
    displayName: 'Run Command',
    icon: 'TERM',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Run a command in a fresh process',
    displayMode: 'standard',
    primaryColor: 'var(--tool-card-terminal-color)'
  },

  'WriteStdin': {
    toolName: 'WriteStdin',
    displayName: 'Write Input',
    icon: 'TERM',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Write to or poll a running command process',
    displayMode: 'standard',
    primaryColor: 'var(--tool-card-terminal-color)'
  },

  'ExecControl': {
    toolName: 'ExecControl',
    displayName: 'Control Process',
    icon: 'TERM',
    requiresConfirmation: false,
    resultDisplayType: 'detailed',
    description: 'Interrupt or kill a running command process',
    displayMode: 'standard',
    primaryColor: 'var(--color-error)'
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
      primaryColor: 'var(--color-purple-500)'
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
    primaryColor: 'var(--color-text-muted)'
  };
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

// ==================== Collapsible explorer tools ====================


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
 * - Text needs context (use isCollapsibleItemWithContext).
 * - Thinking can be collapsed with explorer tools.
 * - Only explorer tools are collapsible.
 */
export function isCollapsibleItem(item: FlowItem): boolean {
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
