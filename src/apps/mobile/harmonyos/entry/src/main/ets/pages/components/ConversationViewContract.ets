import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { ChatSurface } from './ChatSurface';

export class ConversationViewContract {
  static surfaceForRoute(isRemoteChat: boolean): ChatSurface {
    return isRemoteChat ? ChatSurface.Remote : ChatSurface.General;
  }

  static shouldShowSuggestions(
    showSuggestionsWhenEmpty: boolean,
    isBusy: boolean,
    timelineItems: ChatTimelineItem[]
  ): boolean {
    return showSuggestionsWhenEmpty && !isBusy && !ConversationViewContract.hasRealTimelineItem(timelineItems);
  }

  static visibleTimelineItems(timelineItems: ChatTimelineItem[]): ChatTimelineItem[] {
    return timelineItems.filter((item: ChatTimelineItem) => item.type !== 'empty_state');
  }

  static hasRealTimelineItem(timelineItems: ChatTimelineItem[]): boolean {
    return timelineItems.some((item: ChatTimelineItem) => item.type !== 'empty_state');
  }
}
