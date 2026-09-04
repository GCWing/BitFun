import type { FlowItem, FlowToolItem, ModelRound } from '../types/flow-chat';
import { parseCanvasArtifactReference } from '@/shared/utils/canvasArtifactReference';
import { getEffectiveToolName } from './toolInvocationIdentity';

const CANVAS_ARTIFACT_MUTATION_TOOLS = new Set([
  'CreateCanvas',
  'PatchCanvas',
  'UpdateCanvas',
]);

function parseToolResult(result: unknown): Record<string, unknown> | null {
  if (typeof result === 'string') {
    try {
      const parsed = JSON.parse(result);
      return parsed && typeof parsed === 'object' ? parsed as Record<string, unknown> : null;
    } catch {
      return null;
    }
  }
  return result && typeof result === 'object' ? result as Record<string, unknown> : null;
}

export function canvasArtifactReferenceFromToolItem(item: FlowItem): string | null {
  if (item.type !== 'tool') return null;
  const toolItem = item as FlowToolItem;
  if (
    toolItem.status !== 'completed'
    || toolItem.toolResult?.success === false
    || !CANVAS_ARTIFACT_MUTATION_TOOLS.has(getEffectiveToolName(toolItem))
  ) {
    return null;
  }

  const artifactReference = parseToolResult(toolItem.toolResult?.result)?.artifactReference;
  if (typeof artifactReference !== 'string' || !parseCanvasArtifactReference(artifactReference)) {
    return null;
  }
  return artifactReference;
}

export function collectCanvasArtifactToolItems(rounds: ModelRound[]): FlowToolItem[] {
  const latestByReference = new Map<string, FlowToolItem>();

  for (const round of rounds) {
    for (const item of round.items) {
      const artifactReference = canvasArtifactReferenceFromToolItem(item);
      if (!artifactReference) continue;
      // Delete before set so an updated artifact moves to its latest position.
      latestByReference.delete(artifactReference);
      latestByReference.set(artifactReference, item as FlowToolItem);
    }
  }

  return [...latestByReference.values()];
}
