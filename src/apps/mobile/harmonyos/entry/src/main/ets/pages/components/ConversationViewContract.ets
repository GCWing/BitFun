import { ChatTimelineItem } from '../../model/ChatTimelineModels';
import { ConversationMessageRenderPolicy } from '../policy/ConversationMessageRenderPolicy';
import { ChatSurface } from '../state/ChatSurface';
import { toConversationUiMessage } from '../state/ConversationUiModels';
import { ComposerPresentation } from './ComposerBar';

/** Layout and host chrome that vary independently from conversation data. */
export class ConversationViewOptions {
  activeFilePreviewPath: string = '';
  activeFilePreviewLoading: boolean = false;
  showSidebarButton: boolean = true;
  showBackButton: boolean = true;
  showSidebarRestoreButton: boolean = false;
  composerPresentation: ComposerPresentation = ComposerPresentation.Compact;
  composerInputId: string = 'conversation-composer-input';
  hoverOperateMode: boolean = false;
}

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

  static visibleTimelineItems<T extends ChatTimelineItem>(timelineItems: T[]): T[] {
    // Repeat.virtualScroll requires every input entry to create one child. A
    // hollow assistant is deliberately invisible, so passing it through and
    // conditionally omitting its ListItem corrupts the virtual index/height
    // map and can move adjacent user bubbles outside the viewport. Keep the
    // renderability fence at the view contract instead: everything handed to
    // the virtual list is guaranteed to own a visible row.
    return timelineItems.filter((item: T) => {
      if (item.type === 'empty_state' || !item.message) {
        return false;
      }
      return ConversationMessageRenderPolicy.shouldRender(toConversationUiMessage(item.message));
    });
  }

  static hasRealTimelineItem(timelineItems: ChatTimelineItem[]): boolean {
    return timelineItems.some((item: ChatTimelineItem) => item.type !== 'empty_state');
  }
}
