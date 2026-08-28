import { ConversationEvent } from '../../services/ConversationEvent';
import { ChatTimelineItem } from '../../model/ChatTimelineModels';
import { ChatTimelineState, ChatTimelineStore } from '../../services/ChatTimelineStore';

/**
 * Conversation-facing MVVM boundary.
 * The store remains independently testable; the ViewModel is the page-facing
 * owner of projected conversation state and transport-neutral events.
 */
export class ConversationViewModel extends ChatTimelineStore {
  dispatch(event: ConversationEvent): void {
    this.applyEvent(event);
  }

  viewState(hasMoreMessages: boolean): ChatTimelineItem[] {
    return this.project(hasMoreMessages);
  }

  snapshotState(): ChatTimelineState {
    return this.snapshot();
  }
}
