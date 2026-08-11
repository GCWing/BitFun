import { describe, expect, it } from '@ohos/hypium';
import { AppRootHostPort } from '../main/ets/pages/host/AppRootHostAdapter';
import { AppRoute } from '../main/ets/pages/navigation/AppRouteContract';
import { AppRootRuntime } from '../main/ets/pages/runtime/AppRootRuntime';

class FakeAppRootHost implements AppRootHostPort {
  attach(_context: Context, _uiContext: UIContext): void {
  }

  context(): Context {
    return {} as Context;
  }

  animate(_duration: number, callback: () => void): void {
    callback();
  }

  showToast(_message: string, _duration: number): boolean {
    return false;
  }
}

export default function appRootRuntimeStartupUnitTest() {
  describe('AppRootRuntime startup restore', () => {
    it('applies cloud credentials without selecting a remote target', 0, () => {
      const runtime = new AppRootRuntime(new FakeAppRootHost());
      runtime.settingsController.applyCloudAccountSession({
        token: 'token-1',
        userId: 'user-1',
        masterKey: new Uint8Array(32)
      }, 'https://relay.example.com', 'alice');

      expect(runtime.remotePageState.accountUserId).assertEqual('user-1');
      expect(runtime.remotePageState.accountUsername).assertEqual('alice');
      expect(runtime.remotePageState.controlTargetType).assertEqual('none');
      expect(runtime.remotePageState.controlTargetDeviceId).assertEqual('');
      expect(runtime.appShellViewModel.currentRoute()).assertEqual(AppRoute.ChatHome);
    });
  });
}
