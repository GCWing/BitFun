import { describe, expect, it } from '@ohos/hypium';
import { AppRootHostPort } from '../main/ets/pages/host/AppRootHostAdapter';
import { AppRoute } from '../main/ets/pages/navigation/AppRouteContract';
import { AppRootRuntime } from '../main/ets/pages/state/AppRootRuntime';
import { FilePreviewRequest, FilePreviewTarget } from '../main/ets/pages/state/FilePreviewTarget';

class FakeAppRootHost implements AppRootHostPort {
  externalLinks: string[] = [];
  externalLinkResult: boolean = true;

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

  async openExternalLink(link: string): Promise<boolean> {
    this.externalLinks.push(link);
    return this.externalLinkResult;
  }
}

class TestAppRootRuntime extends AppRootRuntime {
  stopGeneralChatStreamCalls: number = 0;

  constructor(host: AppRootHostPort = new FakeAppRootHost()) {
    super(host);
  }

  stopGeneralChatStream(cancelled: boolean, finalStatus: string = 'cancelled'): void {
    this.stopGeneralChatStreamCalls += 1;
    super.stopGeneralChatStream(cancelled, finalStatus);
  }
}

export default function appRootLifecycleUnitTest() {
  describe('AppRootRuntime page hide lifecycle', () => {
    it('keeps backgrounded general chat running on page hide', 0, () => {
      const runtime = new TestAppRootRuntime();

      runtime.onPageHide();

      expect(runtime.stopGeneralChatStreamCalls).assertEqual(0);
    });

    it('still performs general chat cleanup when the app truly disappears', 0, () => {
      const runtime = new TestAppRootRuntime();

      runtime.aboutToDisappear();

      expect(runtime.stopGeneralChatStreamCalls).assertEqual(1);
    });

    it('routes HTTP Markdown links through the host without opening file preview', 0, async () => {
      const host = new FakeAppRootHost();
      const runtime = new TestAppRootRuntime(host);

      runtime.openFilePreview(
        AppRoute.ChatHome,
        new FilePreviewRequest('https://example.com/docs', 'docs')
      );
      await new Promise<void>((resolve: () => void) => setTimeout(resolve, 0));

      expect(host.externalLinks.length).assertEqual(1);
      expect(host.externalLinks[0]).assertEqual('https://example.com/docs');
      expect(runtime.filePreviewState.visible).assertFalse();
    });

    it('closes preview before applying conversation navigation back', 0, () => {
      const runtime = new TestAppRootRuntime();
      runtime.filePreviewState.begin(new FilePreviewTarget(
        'README.md', 'README.md', 'README.md', 'session-1', '/workspace', 1
      ));

      expect(runtime.handleNavigationBack(AppRoute.RemoteChat)).assertTrue();
      expect(runtime.filePreviewState.visible).assertFalse();
      expect(runtime.handleRootBack()).assertFalse();
    });

    it('invalidates and closes preview when the remote target changes', 0, () => {
      const runtime = new TestAppRootRuntime();
      runtime.filePreviewState.begin(new FilePreviewTarget(
        'README.md', 'README.md', 'README.md', 'session-1', '/workspace', 1
      ));

      runtime.invalidateFilePreviewTarget();

      expect(runtime.filePreviewState.visible).assertFalse();
    });

    it('closes preview only when the active remote session ownership changes', 0, () => {
      const runtime = new TestAppRootRuntime();
      runtime.remotePageState.setActiveSession({
        sessionId: 'session-1',
        title: 'Session 1',
        workspacePath: '/workspace',
        agentType: 'code'
      });
      runtime.filePreviewState.begin(new FilePreviewTarget(
        'README.md', 'README.md', 'README.md', 'session-1', '/workspace', 1
      ));

      runtime.applyRemoteActiveSession({
        sessionId: 'session-1',
        title: 'Renamed session',
        workspacePath: '/workspace',
        agentType: 'code'
      });
      expect(runtime.filePreviewState.visible).assertTrue();

      runtime.applyRemoteActiveSession({
        sessionId: 'session-2',
        title: 'Session 2',
        workspacePath: '/workspace',
        agentType: 'code'
      });
      expect(runtime.filePreviewState.visible).assertFalse();
    });
  });
}
