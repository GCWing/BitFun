/**
 * Static metadata for built-in tools displayed in the Tools scene.
 *
 * Each tool points back to the real tool name registered in
 * `src/crates/core/src/agentic/tools/registry.rs`. UI-only concepts
 * (category, permission, icon) live here; human-readable text lives in
 * the `scenes/tools` i18n namespace under `builtin.<ToolName>`.
 */

import type { LucideIcon } from 'lucide-react';
import {
  FileText,
  FolderOpen,
  FilePenLine,
  FilePlus2,
  Trash2,
  Search,
  Folders,
  Terminal,
  TerminalSquare,
  Globe,
  Link2,
  ListChecks,
  NotebookPen,
  Clock,
  MessageCircleQuestion,
  Mouse,
  Users,
  Network,
  Sparkles,
  GitCompareArrows,
  ShieldCheck,
  Boxes,
  ScrollText,
  History,
  MessageSquare,
  Plug,
  Workflow,
} from 'lucide-react';

export type ToolCategory =
  | 'file'
  | 'navigate'
  | 'shell'
  | 'web'
  | 'plan'
  | 'session'
  | 'interact'
  | 'desktop'
  | 'delegate'
  | 'mcpMeta';

export type ToolPermission = 'read' | 'write' | 'danger';

export interface BuiltinToolMeta {
  /** Real tool name as returned by `Tool::name()` in the Rust registry. */
  name: string;
  category: ToolCategory;
  permission: ToolPermission;
  Icon: LucideIcon;
}

/** Ordered to match a user's mental grouping (not the registration order). */
export const BUILTIN_TOOLS: BuiltinToolMeta[] = [
  // File
  { name: 'Read',        category: 'file',     permission: 'read',   Icon: FileText },
  { name: 'Write',       category: 'file',     permission: 'write',  Icon: FilePlus2 },
  { name: 'Edit',        category: 'file',     permission: 'write',  Icon: FilePenLine },
  { name: 'Delete',      category: 'file',     permission: 'danger', Icon: Trash2 },
  { name: 'GetFileDiff', category: 'file',     permission: 'read',   Icon: GitCompareArrows },

  // Navigate
  { name: 'LS',          category: 'navigate', permission: 'read',   Icon: FolderOpen },
  { name: 'Glob',        category: 'navigate', permission: 'read',   Icon: Folders },
  { name: 'Grep',        category: 'navigate', permission: 'read',   Icon: Search },

  // Shell
  { name: 'Bash',            category: 'shell', permission: 'danger', Icon: Terminal },
  { name: 'TerminalControl', category: 'shell', permission: 'write',  Icon: TerminalSquare },

  // Web
  { name: 'WebSearch', category: 'web', permission: 'read', Icon: Globe },
  { name: 'WebFetch',  category: 'web', permission: 'read', Icon: Link2 },

  // Plan
  { name: 'CreatePlan',     category: 'plan', permission: 'write', Icon: NotebookPen },
  { name: 'TodoWrite',      category: 'plan', permission: 'write', Icon: ListChecks },
  { name: 'Cron',           category: 'plan', permission: 'write', Icon: Clock },

  // Session
  { name: 'SessionMessage', category: 'session', permission: 'write', Icon: MessageSquare },
  { name: 'SessionHistory', category: 'session', permission: 'read',  Icon: History },
  { name: 'SessionControl', category: 'session', permission: 'write', Icon: Workflow },
  { name: 'Log',            category: 'session', permission: 'write', Icon: ScrollText },

  // Interact
  { name: 'AskUserQuestion', category: 'interact', permission: 'read',  Icon: MessageCircleQuestion },
  { name: 'GenerativeUI',    category: 'interact', permission: 'read',  Icon: Sparkles },

  // Desktop
  { name: 'ComputerUse', category: 'desktop', permission: 'danger', Icon: Mouse },

  // Delegate
  { name: 'AgentDispatch', category: 'delegate', permission: 'read',   Icon: Network },
  { name: 'Task',          category: 'delegate', permission: 'write',  Icon: Users },
  { name: 'Skill',         category: 'delegate', permission: 'write',  Icon: Boxes },
  { name: 'CodeReview',    category: 'delegate', permission: 'read',   Icon: ShieldCheck },
  { name: 'InitLiveApp',   category: 'delegate', permission: 'write',  Icon: Boxes },

  // MCP meta tools (built-in helpers that inspect MCP capabilities)
  { name: 'ListMCPResources', category: 'mcpMeta', permission: 'read', Icon: Plug },
  { name: 'ReadMCPResource',  category: 'mcpMeta', permission: 'read', Icon: Plug },
  { name: 'ListMCPPrompts',   category: 'mcpMeta', permission: 'read', Icon: Plug },
  { name: 'GetMCPPrompt',     category: 'mcpMeta', permission: 'read', Icon: Plug },
];

export const CATEGORY_ORDER: ToolCategory[] = [
  'file', 'navigate', 'shell', 'web', 'desktop', 'plan', 'session', 'interact', 'delegate', 'mcpMeta',
];

export function countByCategory(): Record<ToolCategory, number> {
  const out = Object.fromEntries(CATEGORY_ORDER.map(c => [c, 0])) as Record<ToolCategory, number>;
  for (const t of BUILTIN_TOOLS) out[t.category] += 1;
  return out;
}
