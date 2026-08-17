import { ConversationUiToolStatus } from '../state/ConversationUiModels';
import { ToolCollapsePolicy } from './ToolCollapsePolicy';

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
   * A group renders every thinking part above every tool it holds, so a
   * thinking entry that arrives after a tool has to open a new group.
   * Folding it into the current one would hoist it above tools that had
   * already run by the time the model wrote it.
   */
  static startsNewActivityGroup(isThinking: boolean, groupHasTool: boolean): boolean {
    return isThinking && groupHasTool;
  }

  static thinkingHeaderCount(thinkingPartCount: number): number {
    if (thinkingPartCount <= 0) {
      return 0;
    }
    return 1;
  }
}
