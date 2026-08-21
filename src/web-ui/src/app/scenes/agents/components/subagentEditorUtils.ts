import type { DynamicToolInfo } from '@/shared/types/agent-api';

export interface SubagentEditorToolInfo {
  name: string;
  description: string;
  isReadonly: boolean;
  needsPermissions?: boolean;
  dynamicInfo?: DynamicToolInfo;
}

export {
  REVIEW_SUBAGENT_OPTIONAL_TOOLS,
  REVIEW_SUBAGENT_RECOMMENDED_TOOLS,
  REVIEW_SUBAGENT_REQUIRED_TOOLS,
  evaluateReviewSubagentToolReadiness,
  type ReviewSubagentToolReadiness,
  type ReviewSubagentToolReadinessResult,
} from '@/shared/services/reviewSubagentCapabilities';

// Rules source mirror: `review` is a semantic marker and never filters the
// tool set. Only `readonly` decides whether writable tools are stripped.
export function filterToolsForReviewMode(
  tools: SubagentEditorToolInfo[],
  _review: boolean,
  readonly: boolean,
): SubagentEditorToolInfo[] {
  if (readonly) {
    return tools.filter((tool) => tool.isReadonly);
  }
  return tools;
}

export interface NormalizeReviewModeStateInput {
  review: boolean;
  readonly: boolean;
  selectedTools: Set<string>;
  availableTools: SubagentEditorToolInfo[];
}

export interface NormalizeReviewModeStateResult {
  readonly: boolean;
  selectedTools: Set<string>;
  removedToolNames: string[];
}

// Rules source mirror: review never touches readonly/tools. Only a readonly
// subagent strips writable tools from the selection.
export function normalizeReviewModeState(
  input: NormalizeReviewModeStateInput,
): NormalizeReviewModeStateResult {
  if (!input.readonly) {
    return {
      readonly: input.readonly,
      selectedTools: new Set(input.selectedTools),
      removedToolNames: [],
    };
  }

  const readonlyToolNames = new Set(
    input.availableTools
      .filter((tool) => tool.isReadonly)
      .map((tool) => tool.name),
  );
  const selectedTools = new Set<string>();
  const removedToolNames: string[] = [];

  input.selectedTools.forEach((toolName) => {
    if (readonlyToolNames.has(toolName)) {
      selectedTools.add(toolName);
    } else {
      removedToolNames.push(toolName);
    }
  });

  return {
    readonly: true,
    selectedTools,
    removedToolNames,
  };
}
