import { splitFilePathAndContent } from '@/shared/utils/partialJsonParser';

import type { DialogTurn, FlowToolItem } from '../types/flow-chat';
import { effectiveToolInvocation, getEffectiveToolName } from './toolInvocationIdentity';

const FILE_MUTATION_TOOLS = new Set([
  'write',
  'edit',
  'multiedit',
  'writefile',
  'editfile',
  'createfile',
  'deletefile',
]);
const OPAQUE_WORKSPACE_TOOLS = new Set([
  'exec',
  'execcommand',
  'execcontrol',
  'git',
  'writestdin',
]);

function normalizeToolName(toolName: string): string {
  return toolName.toLowerCase().replace(/[_-]/g, '');
}

function workspaceRelativePath(filePath: string, workspacePath?: string): string {
  const normalized = filePath.trim().replace(/\\/g, '/');
  const workspace = workspacePath?.trim().replace(/\\/g, '/').replace(/\/+$/, '');
  if (!workspace) {
    return normalized;
  }

  const prefix = `${workspace}/`;
  return normalized.toLowerCase().startsWith(prefix.toLowerCase())
    ? normalized.slice(prefix.length)
    : normalized;
}

function turnsAfterBaseline(turns: DialogTurn[], baselineTurnId?: string | null): DialogTurn[] {
  const baselineIndex = baselineTurnId
    ? turns.findIndex((turn) => turn.id === baselineTurnId)
    : -1;
  return baselineIndex >= 0 ? turns.slice(baselineIndex + 1) : turns;
}

export function collectModifiedFilePathsFromTurns(
  turns: DialogTurn[],
  baselineTurnId?: string | null,
  workspacePath?: string,
): string[] {
  const relevantTurns = turnsAfterBaseline(turns, baselineTurnId);
  const paths = new Set<string>();

  for (const turn of relevantTurns) {
    for (const round of turn.modelRounds) {
      for (const item of round.items) {
        if (item.type !== 'tool') {
          continue;
        }

        const tool = item as FlowToolItem;
        const effective = effectiveToolInvocation(tool.toolName, tool.toolCall?.input);
        if (
          !FILE_MUTATION_TOOLS.has(normalizeToolName(effective.toolName)) ||
          tool.status !== 'completed' ||
          tool.toolResult?.success === false
        ) {
          continue;
        }

        const input = effective.input;
        if (!input || typeof input !== 'object') {
          continue;
        }

        const inputRecord = input as Record<string, unknown>;
        const combinedFilePath = splitFilePathAndContent(inputRecord.payload)?.filePath;
        const filePath = combinedFilePath
          ?? inputRecord.file_path
          ?? inputRecord.filePath
          ?? inputRecord.path;
        if (typeof filePath === 'string' && filePath.trim()) {
          paths.add(workspaceRelativePath(filePath, workspacePath));
        }
      }
    }
  }

  return [...paths];
}

export function hasOpaqueWorkspaceMutationRisk(
  turns: DialogTurn[],
  baselineTurnId?: string | null,
): boolean {
  for (const turn of turnsAfterBaseline(turns, baselineTurnId)) {
    for (const round of turn.modelRounds) {
      for (const item of round.items) {
        if (item.type !== 'tool') {
          continue;
        }
        const tool = item as FlowToolItem;
        if (OPAQUE_WORKSPACE_TOOLS.has(normalizeToolName(getEffectiveToolName(tool)))) {
          return true;
        }
      }
    }
  }
  return false;
}
