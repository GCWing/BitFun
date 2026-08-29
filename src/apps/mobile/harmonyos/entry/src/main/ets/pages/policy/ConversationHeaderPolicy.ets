import { ChatSurface } from '../state/ChatSurface';

export class ConversationHeaderPresentation {
  title: string = '';
  subtitle: string = '';
  fallbackKey: string = 'app.title';
  allowRename: boolean = false;
  showActions: boolean = false;

  constructor(
    title: string,
    subtitle: string,
    fallbackKey: string,
    allowRename: boolean,
    showActions: boolean
  ) {
    this.title = title;
    this.subtitle = subtitle;
    this.fallbackKey = fallbackKey;
    this.allowRename = allowRename;
    this.showActions = showActions;
  }
}

/**
 * One header contract for local and remote chat.
 * View supplies i18n for fallbackKey; this policy only decides structure.
 */
export class ConversationHeaderPolicy {
  static readonly REMOTE_BRAND: string = 'BitFun';

  static present(
    surface: ChatSurface,
    sessionTitle: string,
    desktopName: string,
    workspaceBranch: string,
    hasTimeline: boolean
  ): ConversationHeaderPresentation {
    if (surface === ChatSurface.Remote) {
      return new ConversationHeaderPresentation(
        sessionTitle,
        ConversationHeaderPolicy.remoteSubtitle(desktopName, workspaceBranch),
        'chat.remoteSession',
        true,
        true
      );
    }
    return new ConversationHeaderPresentation(
      sessionTitle,
      '',
      'app.title',
      false,
      hasTimeline
    );
  }

  static remoteSubtitle(desktopName: string, workspaceBranch: string): string {
    if (desktopName.length > 0) {
      return desktopName;
    }
    return workspaceBranch.length > 0 ?
      `${ConversationHeaderPolicy.REMOTE_BRAND} · ${workspaceBranch}` :
      ConversationHeaderPolicy.REMOTE_BRAND;
  }
}
