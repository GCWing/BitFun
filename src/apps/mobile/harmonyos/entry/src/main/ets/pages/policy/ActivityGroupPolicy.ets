import { ConversationUiToolStatus } from '../state/ConversationUiModels';
import { ThinkingPresentationPolicy } from './ThinkingPresentationPolicy';
import { ToolCollapseGroup, ToolCollapsePolicy } from './ToolCollapsePolicy';

/**
 * One thinking block inside an activity group, pinned to the position it
 * occupied in the transcript. `beforeToolIndex` is the number of tools in the
 * group that had already run when the model wrote it, so it is also the index
 * of the tool row this block belongs above.
 */
export interface ActivityThinkingPart {
  beforeToolIndex: number;
  text: string;
  status: string;
  streaming: boolean;
  streamKey: string;
}

/**
 * One row of a folded activity group. `kind` is `'thinking'` or `'group'`, and
 * `index` points into the thinking parts or the tool groups accordingly.
 */
export class ActivityRowPlan {
  readonly kind: string;
  readonly index: number;

  constructor(kind: string, index: number) {
    this.kind = kind;
    this.index = index;
  }
}

export class ActivityGroupPolicy {
  static shouldFoldEntry(
    isThinking: boolean,
    isSubagent: boolean,
    tool?: ConversationUiToolStatus
  ): boolean {
    if (isThinking) {
      return true;
    }
    if (isSubagent || !tool) {
      return false;
    }
    return ToolCollapsePolicy.shouldFoldIntoSummary(tool);
  }

  /**
   * Collapse thinking parts that sit at the same point in the run into one
   * block, so an uninterrupted stretch of reasoning keeps a single header
   * while reasoning on either side of a tool call stays where it happened.
   */
  static mergeThinkingParts(parts: ActivityThinkingPart[]): ActivityThinkingPart[] {
    const merged: ActivityThinkingPart[] = [];
    let index = 0;
    while (index < parts.length) {
      const head = parts[index];
      const texts: string[] = [];
      const statuses: string[] = [];
      const streamings: boolean[] = [];
      let cursor = index;
      while (cursor < parts.length && parts[cursor].beforeToolIndex === head.beforeToolIndex) {
        texts.push(parts[cursor].text);
        statuses.push(parts[cursor].status);
        streamings.push(parts[cursor].streaming);
        cursor += 1;
      }
      merged.push({
        beforeToolIndex: head.beforeToolIndex,
        text: ThinkingPresentationPolicy.mergedText(texts),
        status: ThinkingPresentationPolicy.mergedStatusValue(statuses, streamings),
        streaming: ThinkingPresentationPolicy.mergedStreaming(streamings),
        streamKey: head.streamKey
      });
      index = cursor;
    }
    return merged;
  }

  /**
   * A summary row stands for the whole fold, so the reasoning behind it belongs
   * inside the expansion rather than above the header.
   */
  static hasSummaryRow(groups: ToolCollapseGroup[]): boolean {
    return groups.some((group: ToolCollapseGroup) => group.type === 'summary');
  }

  /**
   * Lay thinking blocks out among tool rows in transcript order: each block sits
   * above the tool that ran after it was written, and blocks written after the
   * last tool trail the group. Thinking is never placed above a summary row,
   * which is the fold's own header.
   */
  static weaveRows(
    groups: ToolCollapseGroup[],
    parts: ActivityThinkingPart[],
    showThinking: boolean
  ): ActivityRowPlan[] {
    const rows: ActivityRowPlan[] = [];
    if (!showThinking) {
      for (let index = 0; index < groups.length; index++) {
        rows.push(new ActivityRowPlan('group', index));
      }
      return rows;
    }
    let next = 0;
    for (let index = 0; index < groups.length; index++) {
      const group = groups[index];
      if (group.type === 'tool') {
        while (next < parts.length && parts[next].beforeToolIndex <= group.startIndex) {
          rows.push(new ActivityRowPlan('thinking', next));
          next += 1;
        }
      }
      rows.push(new ActivityRowPlan('group', index));
    }
    while (next < parts.length) {
      rows.push(new ActivityRowPlan('thinking', next));
      next += 1;
    }
    return rows;
  }

  static thinkingHeaderCount(thinkingPartCount: number): number {
    if (thinkingPartCount <= 0) {
      return 0;
    }
    return 1;
  }
}
