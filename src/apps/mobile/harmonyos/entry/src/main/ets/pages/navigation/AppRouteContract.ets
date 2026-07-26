export enum AppRoute {
  ChatHome = 'ChatHome',
  GeneralChat = 'GeneralChat',
  RemoteHome = 'RemoteHome',
  RemoteChat = 'RemoteChat'
}

export class ChatRouteParam {
  sessionId: string;

  constructor(sessionId: string) {
    this.sessionId = sessionId;
  }
}

export enum AppNavigationBackAction {
  CloseSidebar = 'close_sidebar',
  CloseActiveChat = 'close_active_chat',
  PopRemoteHome = 'pop_remote_home',
  AllowSystem = 'allow_system'
}

export class AppNavigationPathSpec {
  name: AppRoute;
  private readonly sessionId: string;

  constructor(name: AppRoute, sessionId: string = '') {
    this.name = name;
    this.sessionId = sessionId;
  }

  hasSessionParam(): boolean {
    return this.sessionId.length > 0;
  }

  routeParam(): ChatRouteParam {
    return new ChatRouteParam(this.sessionId);
  }
}

export class AppRouteContract {
  static currentRoute(pathNames: string[]): AppRoute {
    if (pathNames.length === 0) {
      return AppRoute.ChatHome;
    }
    return pathNames[pathNames.length - 1] as AppRoute;
  }

  static isGeneralComposerRoute(route: AppRoute): boolean {
    return route === AppRoute.ChatHome || route === AppRoute.GeneralChat;
  }

  static isSessionRoute(route: AppRoute): boolean {
    return route === AppRoute.GeneralChat || route === AppRoute.RemoteChat;
  }

  static routeParam(sessionId: string): ChatRouteParam {
    return new ChatRouteParam(sessionId);
  }

  static pathSpec(currentRoute: AppRoute, route: AppRoute, sessionId: string = ''): AppNavigationPathSpec | undefined {
    if (currentRoute === route) {
      return undefined;
    }
    return new AppNavigationPathSpec(route, sessionId);
  }

  static backAction(route: AppRoute, isSidebarOpen: boolean): AppNavigationBackAction {
    if (isSidebarOpen) {
      return AppNavigationBackAction.CloseSidebar;
    }
    if (route === AppRoute.GeneralChat || route === AppRoute.RemoteChat) {
      return AppNavigationBackAction.CloseActiveChat;
    }
    if (route === AppRoute.RemoteHome) {
      return AppNavigationBackAction.PopRemoteHome;
    }
    return AppNavigationBackAction.AllowSystem;
  }
}
