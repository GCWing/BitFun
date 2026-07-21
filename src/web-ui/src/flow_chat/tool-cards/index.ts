/**
 * Tool card registry.
 * Maps tool configs to components.
 */

import { createLogger } from '@/shared/utils/logger';
import { isMcpToolName } from '@/infrastructure/mcp/toolName';
export {
  TOOL_CARD_CONFIGS,
  getToolCardConfig,
  requiresConfirmation,
  getAllToolNames,
  COLLAPSIBLE_TOOL_NAMES,
  READ_TOOL_NAMES,
  SEARCH_TOOL_NAMES,
  COMMAND_TOOL_NAMES,
  isCollapsibleTool,
  isCollapsibleItem,
  isCollapsibleItemWithContext,
} from './toolCardMetadata';

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
import { WebFetchCard } from './WebFetchCard';
import { GetToolSpecCard } from './GetToolSpecCard';
import { ContextCompressionDisplay } from './ContextCompressionDisplay';
import { MCPToolDisplay } from './MCPToolDisplay';
import { SkillDisplay } from './SkillDisplay';
import { AskUserQuestionCard } from './AskUserQuestionCard';
import { GitToolDisplay } from './GitToolDisplay';
import { GetFileDiffDisplay } from './GetFileDiffDisplay';
import { CreatePlanDisplay } from './CreatePlanDisplay';
import { TerminalToolCard } from './TerminalToolCard';
import { ExecCommandToolCard } from './ExecCommandToolCard';
import { WriteStdinToolCard } from './WriteStdinToolCard';
import { ExecControlToolCard } from './ExecControlToolCard';
import { TerminalControlDisplay } from './TerminalControlDisplay';
import { InitMiniAppDisplay } from './MiniAppToolDisplay';
import { PageDeployDisplay } from './PageDeployToolDisplay';
import { PagePublishDisplay } from './PagePublishToolDisplay';
import { GenerativeWidgetToolCard } from './GenerativeWidgetToolCard';
import { CanvasToolCard } from './CanvasToolCard';
import { ReviewSessionSummaryCard } from './ReviewSessionSummaryCard';
import { SessionControlToolCard } from './SessionControlToolCard';
import { SessionMessageToolCard } from './SessionMessageToolCard';
import { ComputerUseToolCard } from './ComputerUseToolCard';

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
  'WebFetch': WebFetchCard,
  
  // Advanced tools
  'Task': TaskToolDisplay,
  'LaunchReviewAgent': TaskToolDisplay,
  'TodoWrite': TodoWriteDisplay,
  
  'submit_code_review': CodeReviewToolCard,
  
  // Context compression
  'ContextCompression': ContextCompressionDisplay,
  'GetToolSpec': GetToolSpecCard,

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

  // Exec process tools
  'ExecCommand': ExecCommandToolCard,
  'WriteStdin': WriteStdinToolCard,
  'ExecControl': ExecControlToolCard,

  // MiniApp tool
  'InitMiniApp': InitMiniAppDisplay,

  // BitFun Page (session-only publish)
  'PageDeploy': PageDeployDisplay,
  'PagePublish': PagePublishDisplay,

  // Generative widget tool
  'GenerativeUI': GenerativeWidgetToolCard,

  // Computer use (desktop automation)
  'ComputerUse': ComputerUseToolCard,

  // BitFun Canvas tools
  'CreateCanvas': CanvasToolCard,
  'ReadCanvas': CanvasToolCard,
  'UpdateCanvas': CanvasToolCard,
  'PatchCanvas': CanvasToolCard,
};

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
