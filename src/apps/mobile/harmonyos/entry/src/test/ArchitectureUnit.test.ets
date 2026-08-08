import { describe, it, expect } from '@ohos/hypium';
import { AsyncLifecycleGate } from '../main/ets/services/AsyncLifecycleGate';
import { ConversationEvent } from '../main/ets/services/ConversationEvent';
import { ChatTimelineStore } from '../main/ets/services/ChatTimelineStore';
import { ConversationViewState } from '../main/ets/pages/state/ConversationViewState';
import { RemotePageState } from '../main/ets/pages/state/RemotePageState';
import { GeneralChatPageState } from '../main/ets/pages/state/GeneralChatPageState';
import { AppRoute } from '../main/ets/pages/navigation/AppRouteContract';
import { ChatMessage } from '../main/ets/model/RemoteModels';
import { AppShellViewModel } from '../main/ets/pages/viewmodel/AppShellViewModel';
import { AppNavigationBackAction } from '../main/ets/pages/navigation/AppRouteContract';
import { WideLayoutGeometry } from '../main/ets/pages/layout/WideLayoutGeometry';

export default function architectureUnitTest() {
  describe('MobileArchitecture', () => {
    it('invalidates asynchronous work by generation', 0, () => {
      const gate = new AsyncLifecycleGate();
      const first = gate.begin();
      expect(gate.isCurrent(first)).assertTrue();
      gate.invalidate();
      expect(gate.isCurrent(first)).assertFalse();
    });

    it('reduces active turn events through the shared timeline', 0, () => {
      const store = new ChatTimelineStore();
      const message: ChatMessage = {
        id: 'active-turn',
        turnId: 'turn-1',
        role: 'assistant',
        text: 'streamed',
        status: 'streaming',
        detail: '',
        timestamp: '2026-01-01T00:00:00.000Z',
        items: []
      };
      const event: ConversationEvent = {
        type: 'active_turn_updated',
        sessionId: 'session-1',
        message
      };
      store.applyEvent(event);
      expect(store.snapshot().activeTurn?.text || '').assertEqual('streamed');
    });

    it('projects local and remote conversations through one view state', 0, () => {
      const store = new ChatTimelineStore();
      const remotePage = new RemotePageState();
      const generalPage = new GeneralChatPageState();
      remotePage.setActiveSession({ sessionId: 'remote-1', title: '', workspacePath: '', agentType: 'code' });
      generalPage.setActiveSession({ sessionId: 'general-1', title: '', workspacePath: '', agentType: 'chat' });
      const remote = ConversationViewState.project(AppRoute.RemoteChat, remotePage, generalPage, '');
      const general = ConversationViewState.project(AppRoute.GeneralChat, remotePage, generalPage, '');
      expect(remote.activeSession.sessionId).assertEqual('remote-1');
      expect(general.activeSession.sessionId).assertEqual('general-1');
    });

    it('owns navigation and global overlay state in AppShellViewModel', 0, () => {
      const shell = new AppShellViewModel();
      shell.pushRoute(AppRoute.RemoteHome);
      expect(shell.currentRoute()).assertEqual(AppRoute.RemoteHome);
      shell.state.setSidebarVisible(true);
      expect(shell.backAction(AppRoute.RemoteHome)).assertEqual(AppNavigationBackAction.CloseSidebar);
      shell.state.setSidebarVisible(false);
      shell.replaceRoute(AppRoute.GeneralChat, 'session-1');
      expect(shell.currentRoute()).assertEqual(AppRoute.GeneralChat);
      expect(shell.isGeneralChatVisible()).assertTrue();
    });

    it('replaces conversation sources without growing cross-source history', 0, () => {
      const shell = new AppShellViewModel();
      shell.pushRoute(AppRoute.GeneralChat, 'general-1');
      shell.pushRoute(AppRoute.RemoteHome);
      expect(shell.navigationStack.getAllPathName().length).assertEqual(2);

      shell.replaceRouteWithoutAnimation(AppRoute.RemoteChat, 'remote-1');
      expect(shell.currentRoute()).assertEqual(AppRoute.RemoteChat);
      expect(shell.navigationStack.getAllPathName().length).assertEqual(1);

      shell.replaceRouteWithoutAnimation(AppRoute.ChatHome);
      expect(shell.currentRoute()).assertEqual(AppRoute.ChatHome);
      expect(shell.navigationStack.getAllPathName().length).assertEqual(0);

      shell.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
      expect(shell.currentRoute()).assertEqual(AppRoute.RemoteHome);
      expect(shell.navigationStack.getAllPathName().length).assertEqual(1);
    });

    it('keeps same-route state updates from rebuilding navigation', 0, () => {
      const shell = new AppShellViewModel();
      shell.pushRoute(AppRoute.RemoteHome);
      shell.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
      expect(shell.currentRoute()).assertEqual(AppRoute.RemoteHome);
      expect(shell.navigationStack.getAllPathName().length).assertEqual(1);
    });

    it('keeps wide layout geometry pure and deterministic', 0, () => {
      expect(WideLayoutGeometry.detailOffset(false, 24, 8)).assertEqual(24);
      expect(WideLayoutGeometry.detailOffset(true, 24, 8)).assertEqual(8);
      expect(WideLayoutGeometry.detailWidth(true, 900, 1200)).assertEqual(1200);
      expect(WideLayoutGeometry.collapsedVisualBias(true, 0, 1100, 920, 72)).assertEqual(72);
      expect(WideLayoutGeometry.collapsedVisualBias(false, 0, 1100, 920, 72)).assertEqual(0);
      expect(WideLayoutGeometry.areaLength('1080')).assertEqual(1080);
      expect(WideLayoutGeometry.areaLength('invalid')).assertEqual(0);
    });
  });
}
