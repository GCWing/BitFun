import { ChatTimelineItem } from '../../model/ChatTimelineModels';
import { RecentWorkspaceEntry } from '../../model/RemoteModels';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { ChatSurface } from '../state/ChatSurface';
import { RemoteHarnessProfile } from '../state/RemoteCreateSessionState';
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
  showRemoteCreateContext: boolean = false;
}

/** Actions that only exist on the remote-session creation surface. */
export class RemoteCreateViewActions {
  toggleDeviceMenu: () => void = () => {};
  toggleWorkspaceMenu: () => void = () => {};
  selectDevice: (device: CloudAccountDevice) => void = (_device: CloudAccountDevice) => {};
  selectWorkspace: (workspace?: RecentWorkspaceEntry) => void = (_workspace?: RecentWorkspaceEntry) => {};
  selectHarness: (profile: RemoteHarnessProfile) => void = (_profile: RemoteHarnessProfile) => {};
  backgroundTap: () => void = () => {};
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
    return timelineItems.filter((item: T) => item.type !== 'empty_state');
  }

  static hasRealTimelineItem(timelineItems: ChatTimelineItem[]): boolean {
    return timelineItems.some((item: ChatTimelineItem) => item.type !== 'empty_state');
  }
}
