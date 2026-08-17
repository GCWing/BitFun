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

  static thinkingHeaderCount(thinkingPartCount: number): number {
    if (thinkingPartCount <= 0) {
      return 0;
    }
    return 1;
  }
}
