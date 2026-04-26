export interface SubagentEditorToolInfo {
  name: string;
  isReadonly: boolean;
}

export function filterToolsForReviewMode(
  tools: SubagentEditorToolInfo[],
  review: boolean,
): SubagentEditorToolInfo[] {
  return review ? tools.filter((tool) => tool.isReadonly) : tools;
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

export function normalizeReviewModeState(
  input: NormalizeReviewModeStateInput,
): NormalizeReviewModeStateResult {
  if (!input.review) {
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
