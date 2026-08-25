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
import { SidebarDirectoryPreviewPolicy } from '../main/ets/pages/policy/SidebarDirectoryPreviewPolicy';
import { AdaptiveLayoutInput } from '../main/ets/pages/policy/AdaptiveLayoutInput';
import {
  ConversationLayoutCrease,
  ConversationLayoutHorizontalCrease,
  ConversationLayoutPolicy,
  ConversationLayoutRect
} from '../main/ets/pages/policy/ConversationLayoutPolicy';
import {
  SettingsPlacement,
  SettingsPlacementMode,
  SettingsPlacementPolicy,
  SettingsSheetKind
} from '../main/ets/pages/policy/SettingsPlacementPolicy';
import { AdaptiveSheetOptions } from '../main/ets/pages/components/AdaptiveSheetOptions';

function placementInput(
  viewportWidth: number,
  viewportHeight: number,
  isFolded: boolean,
  isHoverOperate: boolean,
  wideLayoutMatched: boolean,
  verticalCreases: ConversationLayoutCrease[],
  horizontalCreases: ConversationLayoutHorizontalCrease[],
  isExpandedFoldable: boolean = false,
  foldStatus: number = 0,
  isRtl: boolean = false
): AdaptiveLayoutInput {
  return new AdaptiveLayoutInput(
    viewportWidth,
    viewportHeight,
    foldStatus,
    isFolded,
    isExpandedFoldable,
    isHoverOperate,
    wideLayoutMatched,
    verticalCreases,
    horizontalCreases,
    isRtl
  );
}

function assertSheetMissesCreases(
  placement: SettingsPlacement,
  creases: ConversationLayoutCrease[],
  viewportWidth: number
): void {
  creases.forEach((crease: ConversationLayoutCrease): void => {
    expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(placement, crease, viewportWidth))
      .assertFalse();
  });
}

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

    // Switching sessions is a state change, not a navigation. Treating the
    // session id as part of the destination made every row tap clear the stack
    // and push a fresh one, and on the wide layout the master pane lives inside
    // that destination — so the whole sidebar was rebuilt on each tap.
    it('keeps a session switch from rebuilding the chat destination', 0, () => {
      const shell = new AppShellViewModel();
      shell.pushRoute(AppRoute.RemoteHome);
      shell.pushRoute(AppRoute.RemoteChat, 'session-a');
      shell.replaceRouteWithoutAnimation(AppRoute.RemoteChat, 'session-b');
      expect(shell.currentRoute()).assertEqual(AppRoute.RemoteChat);
      expect(shell.navigationStack.getAllPathName().length).assertEqual(2);
    });

    it('keeps wide layout geometry pure and deterministic', 0, () => {
      expect(WideLayoutGeometry.detailOffset(false, 24, 8)).assertEqual(24);
      expect(WideLayoutGeometry.detailOffset(true, 24, 8)).assertEqual(8);
      expect(WideLayoutGeometry.detailWidth(true, 900, 1200)).assertEqual(1200);
      expect(WideLayoutGeometry.areaLength('1080')).assertEqual(1080);
      expect(WideLayoutGeometry.areaLength('invalid')).assertEqual(0);
    });

    it('reveals three more children per tap instead of the whole remainder', 0, () => {
      const workspaces = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
      expect(SidebarDirectoryPreviewPolicy.visibleSlice(workspaces, 3).join(','))
        .assertEqual('a,b,c');
      expect(SidebarDirectoryPreviewPolicy.hiddenCount(workspaces.length, 3)).assertEqual(5);
      expect(SidebarDirectoryPreviewPolicy.nextVisibleCount(3, 8)).assertEqual(6);
      expect(SidebarDirectoryPreviewPolicy.visibleSlice(workspaces, 6).join(','))
        .assertEqual('a,b,c,d,e,f');
      expect(SidebarDirectoryPreviewPolicy.nextVisibleCount(6, 8)).assertEqual(8);
      expect(SidebarDirectoryPreviewPolicy.nextVisibleCount(8, 8)).assertEqual(8);
      expect(SidebarDirectoryPreviewPolicy.hiddenCount(2, 3)).assertEqual(0);
      expect(SidebarDirectoryPreviewPolicy.workspaceExpandedByDefault(true)).assertFalse();
      expect(SidebarDirectoryPreviewPolicy.workspaceExpandedByDefault(false)).assertTrue();
    });
  });

  describe('SettingsPlacementPolicy', () => {
    it('keeps a portrait phone and a folded cover on a large bottom sheet', 0, () => {
      const phone = SettingsPlacementPolicy.resolve(
        placementInput(390, 844, false, false, false, [], []),
        SettingsSheetKind.Settings
      );
      const folded = SettingsPlacementPolicy.resolve(
        placementInput(390, 844, true, false, false, [], [], false, 2),
        SettingsSheetKind.Connect
      );
      const empty = SettingsPlacementPolicy.resolve(
        AdaptiveLayoutInput.empty(),
        SettingsSheetKind.Settings
      );
      expect(phone.mode).assertEqual(SettingsPlacementMode.Bottom);
      expect(phone.width).assertEqual(0);
      expect(phone.height).assertEqual(0);
      expect(folded.mode).assertEqual(SettingsPlacementMode.Bottom);
      expect(empty.mode).assertEqual(SettingsPlacementMode.Bottom);
      expect(SettingsPlacementPolicy.compactBottom().mode).assertEqual(SettingsPlacementMode.Bottom);
      expect(SettingsPlacementPolicy.compactBottom(SettingsSheetKind.RemoteViewSettings).height)
        .assertEqual(SettingsPlacementPolicy.REMOTE_VIEW_SETTINGS_HEIGHT);
      assertSheetMissesCreases(phone, [], 390);
    });

    it('uses a 45 percent side sheet when a narrow window is landscape', 0, () => {
      const placement = SettingsPlacementPolicy.resolve(
        placementInput(844, 390, false, false, false, [], []),
        SettingsSheetKind.Settings
      );
      expect(placement.mode).assertEqual(SettingsPlacementMode.Side);
      expect(placement.width).assertEqual(Math.round(844 * 0.45));
      expect(placement.height).assertEqual(SettingsPlacementPolicy.SIDE_MIN_HEIGHT);
      expect(SettingsPlacementPolicy.resolve(
        placementInput(590, 300, false, false, false, [], []),
        SettingsSheetKind.Settings
      ).mode).assertEqual(SettingsPlacementMode.Bottom);
      assertSheetMissesCreases(placement, [], 844);
    });

    it('clamps a crease-free tablet sheet between 400 and 520', 0, () => {
      const pad = SettingsPlacementPolicy.resolve(
        placementInput(1440, 960, false, false, true, [], [], false, 0),
        SettingsSheetKind.Settings
      );
      const narrowWide = SettingsPlacementPolicy.resolve(
        placementInput(1000, 800, false, false, true, [], []),
        SettingsSheetKind.Connect
      );
      const huge = SettingsPlacementPolicy.resolve(
        placementInput(2000, 1200, false, false, true, [], []),
        SettingsSheetKind.Settings
      );
      expect(pad.mode).assertEqual(SettingsPlacementMode.Side);
      expect(pad.width).assertEqual(461);
      expect(pad.height).assertEqual(SettingsPlacementPolicy.SIDE_MAX_HEIGHT);
      expect(narrowWide.width).assertEqual(SettingsPlacementPolicy.TABLET_MIN_WIDTH);
      expect(huge.width).assertEqual(SettingsPlacementPolicy.TABLET_MAX_WIDTH);
      expect(SettingsPlacementPolicy.resolve(
        placementInput(700, 900, false, false, false, [], []),
        SettingsSheetKind.Settings
      ).mode).assertEqual(SettingsPlacementMode.Side);
      expect(SettingsPlacementPolicy.resolve(
        placementInput(0, 0, false, false, true, [], []),
        SettingsSheetKind.Settings
      ).mode).assertEqual(SettingsPlacementMode.Bottom);
      expect(SettingsPlacementPolicy.resolve(
        placementInput(1440, 0, false, false, true, [], []),
        SettingsSheetKind.Settings
      ).height).assertEqual(SettingsPlacementPolicy.SIDE_MAX_HEIGHT);
      assertSheetMissesCreases(pad, [], 1440);
    });

    it('sizes a dual-fold sheet to the pane after the only vertical crease', 0, () => {
      const crease = new ConversationLayoutCrease(352, 16);
      const placement = SettingsPlacementPolicy.resolve(
        placementInput(680, 740, false, false, true, [crease], [], true, 1),
        SettingsSheetKind.Settings
      );
      expect(placement.mode).assertEqual(SettingsPlacementMode.Side);
      expect(placement.width).assertEqual(312);
      expect(placement.height).assertEqual(660);
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(placement, crease, 680))
        .assertFalse();
      assertSheetMissesCreases(placement, [crease], 680);
    });

    it('sizes a tri-fold sheet to the pane after the last vertical crease', 0, () => {
      const creases = [
        new ConversationLayoutCrease(351, 44),
        new ConversationLayoutCrease(773, 44)
      ];
      const placement = SettingsPlacementPolicy.resolve(
        placementInput(1107, 776, false, false, true, creases, [], true, 11),
        SettingsSheetKind.SessionDetails
      );
      expect(placement.mode).assertEqual(SettingsPlacementMode.Side);
      expect(placement.width).assertEqual(290);
      expect(placement.height).assertEqual(SettingsPlacementPolicy.SESSION_DETAILS_HEIGHT);
      assertSheetMissesCreases(placement, creases, 1107);
    });

    it('keeps an extremely narrow crease pane instead of applying a width floor', 0, () => {
      const crease = new ConversationLayoutCrease(590, 8);
      const placement = SettingsPlacementPolicy.resolve(
        placementInput(630, 700, false, false, true, [crease], [], true, 1),
        SettingsSheetKind.Settings
      );
      expect(placement.mode).assertEqual(SettingsPlacementMode.Side);
      expect(placement.width).assertEqual(32);
      expect(placement.width < SettingsPlacementPolicy.TABLET_MIN_WIDTH).assertTrue();
      assertSheetMissesCreases(placement, [crease], 630);
    });

    it('falls back to the crease-free decision tree when crease data is illegal', 0, () => {
      const illegal = [
        new ConversationLayoutCrease(-12, 8),
        new ConversationLayoutCrease(420, -2),
        new ConversationLayoutCrease(910, 8)
      ];
      const placement = SettingsPlacementPolicy.resolve(
        placementInput(900, 800, false, false, true, illegal, [], true, 1),
        SettingsSheetKind.RemoteViewSettings
      );
      const overflow = SettingsPlacementPolicy.resolve(
        placementInput(390, 844, false, false, false, illegal, []),
        SettingsSheetKind.Settings
      );
      expect(placement.mode).assertEqual(SettingsPlacementMode.Side);
      expect(placement.width).assertEqual(SettingsPlacementPolicy.TABLET_MIN_WIDTH);
      expect(placement.width <= 900).assertTrue();
      expect(placement.height).assertEqual(SettingsPlacementPolicy.REMOTE_VIEW_SETTINGS_HEIGHT);
      expect(overflow.mode).assertEqual(SettingsPlacementMode.Bottom);
      expect(overflow.width).assertEqual(0);
      assertSheetMissesCreases(placement, illegal, 900);
      expect(SettingsPlacementPolicy.sheetLeft(
        new SettingsPlacement(SettingsPlacementMode.Side, 312, 560, 0),
        680
      )).assertEqual(368);
      expect(SettingsPlacementPolicy.sheetLeft(
        new SettingsPlacement(SettingsPlacementMode.Side, 312, 560, 0),
        680,
        true
      )).assertEqual(0);
      expect(SettingsPlacementPolicy.sheetLeft(
        new SettingsPlacement(SettingsPlacementMode.Bottom, 0, 0, 0),
        680
      )).assertEqual(0);
      expect(SettingsPlacementPolicy.sheetLeft(
        new SettingsPlacement(SettingsPlacementMode.Side, 0, 560, 0),
        680
      )).assertEqual(0);
      expect(SettingsPlacementPolicy.sheetLeft(
        new SettingsPlacement(SettingsPlacementMode.Side, 400, 560, 0),
        0
      )).assertEqual(0);
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
        new SettingsPlacement(SettingsPlacementMode.Bottom, 0, 0, 0),
        new ConversationLayoutCrease(200, 8),
        900
      )).assertFalse();
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
        new SettingsPlacement(SettingsPlacementMode.Side, 0, 560, 0),
        new ConversationLayoutCrease(200, 8),
        900
      )).assertFalse();
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
        new SettingsPlacement(SettingsPlacementMode.Side, 400, 560, 0),
        new ConversationLayoutCrease(200, 8),
        0
      )).assertFalse();
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
        new SettingsPlacement(SettingsPlacementMode.Side, 500, 560, 0),
        new ConversationLayoutCrease(352, 16),
        680
      )).assertTrue();
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
        new SettingsPlacement(SettingsPlacementMode.Side, 400, 560, 0),
        new ConversationLayoutCrease(200, 8),
        900,
        true
      )).assertTrue();
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
        new SettingsPlacement(SettingsPlacementMode.Side, 312, 560, 0),
        new ConversationLayoutCrease(352, 16),
        680,
        true
      )).assertFalse();
    });

    it('caps a hover sheet to the operate pane below the horizontal crease', 0, () => {
      const hover = SettingsPlacementPolicy.resolve(
        placementInput(
          390,
          844,
          false,
          true,
          false,
          [],
          [new ConversationLayoutHorizontalCrease(400, 16)],
          true,
          3
        ),
        SettingsSheetKind.Settings
      );
      const missing = SettingsPlacementPolicy.resolve(
        placementInput(390, 844, false, true, false, [], [], true, 3),
        SettingsSheetKind.Connect
      );
      const invalid = SettingsPlacementPolicy.resolve(
        placementInput(
          390,
          844,
          false,
          true,
          false,
          [],
          [new ConversationLayoutHorizontalCrease(0, 16), new ConversationLayoutHorizontalCrease(840, 20)],
          true,
          3
        ),
        SettingsSheetKind.SessionDetails
      );
      const mateX7Rect = new ConversationLayoutRect(0, 340.48, 798.72, 54.72);
      const mateX7Creases = ConversationLayoutPolicy.classifyCreaseRects(
        707.2,
        706.56,
        [mateX7Rect]
      );
      const bookHover = SettingsPlacementPolicy.resolve(
        placementInput(
          707.2,
          706.56,
          false,
          true,
          true,
          mateX7Creases.vertical,
          mateX7Creases.horizontal,
          true,
          3
        ),
        SettingsSheetKind.Settings
      );
      const rtlTriFold = SettingsPlacementPolicy.resolve(
        placementInput(
          1107,
          776,
          false,
          false,
          true,
          [
            new ConversationLayoutCrease(351, 44),
            new ConversationLayoutCrease(773, 44)
          ],
          [],
          true,
          11,
          true
        ),
        SettingsSheetKind.Settings
      );
      const remote = SettingsPlacementPolicy.resolve(
        placementInput(
          390,
          844,
          false,
          true,
          false,
          [],
          [new ConversationLayoutHorizontalCrease(200, 16)],
          true,
          3
        ),
        SettingsSheetKind.RemoteViewSettings
      );
      const details = SettingsPlacementPolicy.resolve(
        placementInput(
          390,
          844,
          false,
          true,
          false,
          [],
          [
            new ConversationLayoutHorizontalCrease(80, 16),
            new ConversationLayoutHorizontalCrease(200, 16)
          ],
          true,
          3
        ),
        SettingsSheetKind.SessionDetails
      );
      expect(hover.mode).assertEqual(SettingsPlacementMode.FoldOperate);
      expect(hover.maxHeight).assertEqual(428);
      expect(hover.height).assertEqual(428);
      expect(missing.mode).assertEqual(SettingsPlacementMode.Bottom);
      expect(missing.maxHeight).assertEqual(0);
      expect(invalid.mode).assertEqual(SettingsPlacementMode.Bottom);
      expect(invalid.maxHeight).assertEqual(0);
      expect(mateX7Creases.vertical.length).assertEqual(1);
      expect(mateX7Creases.horizontal.length).assertEqual(0);
      expect(bookHover.mode).assertEqual(SettingsPlacementMode.Side);
      expect(bookHover.width).assertEqual(312);
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
        bookHover,
        mateX7Creases.vertical[0],
        707.2
      )).assertFalse();
      expect(rtlTriFold.mode).assertEqual(SettingsPlacementMode.Side);
      expect(rtlTriFold.width).assertEqual(351);
      expect(SettingsPlacementPolicy.sheetLeft(rtlTriFold, 1107, true)).assertEqual(0);
      expect(SettingsPlacementPolicy.sheetIntersectsVerticalCrease(
        rtlTriFold,
        new ConversationLayoutCrease(351, 44),
        1107,
        true
      )).assertFalse();
      expect(remote.height).assertEqual(SettingsPlacementPolicy.REMOTE_VIEW_SETTINGS_HEIGHT);
      expect(details.maxHeight).assertEqual(748);
      expect(details.height).assertEqual(SettingsPlacementPolicy.SESSION_DETAILS_HEIGHT);
      expect(AdaptiveSheetOptions.fromPlacement(remote).height)
        .assertEqual(SettingsPlacementPolicy.REMOTE_VIEW_SETTINGS_HEIGHT);
      expect(AdaptiveSheetOptions.fromPlacement(details).height)
        .assertEqual(SettingsPlacementPolicy.SESSION_DETAILS_HEIGHT);
      expect(AdaptiveSheetOptions.fromPlacement(hover).height).assertEqual(428);
      const sideOptions = AdaptiveSheetOptions.fromPlacement(bookHover);
      expect(sideOptions.width).assertEqual(312);
      expect(sideOptions.preferType).assertEqual(SheetType.SIDE);
      expect(AdaptiveSheetOptions.fromPlacement(
        SettingsPlacementPolicy.compactBottom(SettingsSheetKind.RemoteViewSettings)
      ).height).assertEqual(SettingsPlacementPolicy.REMOTE_VIEW_SETTINGS_HEIGHT);
      expect(AdaptiveSheetOptions.fromPlacement(
        SettingsPlacementPolicy.compactBottom()
      ).height).assertEqual(SheetSize.LARGE);
      assertSheetMissesCreases(hover, [], 390);
    });
  });
}
