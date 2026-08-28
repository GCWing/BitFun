export type RemoteControlSettingsPage = 'remote' | 'login' | 'profile';

export class RemoteControlSettingsPagePolicy {
  static resolve(
    openAccountOnAppear: boolean,
    showLogin: boolean,
    showProfile: boolean,
    authenticated: boolean
  ): RemoteControlSettingsPage {
    const accountPageRequested = openAccountOnAppear || showLogin || showProfile;
    if (!accountPageRequested) {
      return 'remote';
    }
    return authenticated && !showLogin ? 'profile' : 'login';
  }
}
