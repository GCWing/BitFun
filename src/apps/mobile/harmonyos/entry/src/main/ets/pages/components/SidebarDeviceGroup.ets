import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { DeviceDirectoryEntry } from '../state/DeviceDirectoryState';
import {
  SessionListInputs,
  SessionListProjection,
  SessionListProjectionCache
} from '../policy/SessionListProjection';
import { SidebarAgentProjectionPolicy } from '../policy/SidebarAgentProjectionPolicy';
import { GREEN, INK, MUTED, SOFT, SUBTLE } from './Theme';

/**
 * One desktop and its user-facing Agent instances.
 *
 * Workspaces are execution context, not another permanent navigation level.
 * Tapping the desktop opens the workspace picker; persisted RemoteSession
 * records stay on the wire but project here as Agent instances.
 */
@ComponentV2
export struct SidebarDeviceGroup {
  @Param device: DeviceDirectoryEntry = new DeviceDirectoryEntry();
  @Param sessions: RemoteSession[] = [];
  @Param recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Param workspaceName: string = '';
  @Param workspacePath: string = '';
  @Param workspaceKind: string = 'normal';
  @Param selectedSessionId: string = '';
  @Param runningSessionId: string = '';
  @Param query: string = '';
  @Param showHeader: boolean = false;
  @Param isActive: boolean = false;
  @Event onRetry: () => void = () => {};
  @Event onChooseWorkspace: () => void = () => {};
  @Event onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onSessionActions: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @Event onCreateInWorkspace: (path: string) => void = (_path: string) => {};
  private readonly projectionCache: SessionListProjectionCache = new SessionListProjectionCache();

  build() {
    Column() {
      if (this.showHeader) {
        this.DeviceHeader()
      }
      if (this.shouldShowAgents()) {
        if (this.device.status === 'failed') {
          this.FailedRow()
        } else if (this.device.status === 'loading' && this.agentSessions().length === 0) {
          this.LoadingRow()
        } else {
          this.AgentSection()
        }
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
    .margin({ bottom: this.showHeader ? 10 : 0 })
  }

  @Builder
  private DeviceHeader() {
    Row({ space: 10 }) {
      SymbolGlyph($r('sys.symbol.desktop'))
        .fontSize(19)
        .fontColor([this.device.online ? INK : SUBTLE])
        .width(21)
        .height(21)
        .hitTestBehavior(HitTestMode.None)
      Column({ space: 2 }) {
        Text(this.device.deviceName)
          .width('100%')
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(this.device.online ? INK : SUBTLE)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(this.deviceContextLabel())
          .width('100%')
          .fontSize(12)
          .fontColor(SUBTLE)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      if (!this.device.online) {
        Text(RemoteI18n.t('sidebar.deviceOffline'))
          .fontSize(12)
          .fontColor(SUBTLE)
          .hitTestBehavior(HitTestMode.None)
      }
      if (this.device.status === 'loading') {
        LoadingProgress()
          .width(14)
          .height(14)
          .color(MUTED)
          .hitTestBehavior(HitTestMode.None)
      }
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.chevron_right'))
          .fontSize(13)
          .fontColor([MUTED])
          .hitTestBehavior(HitTestMode.None)
      }
      .width(16)
      .height(16)
      .hitTestBehavior(HitTestMode.None)
    }
    .width('100%')
    .height(54)
    .padding({ left: 10, right: 6 })
    .alignItems(VerticalAlign.Center)
    .opacity(this.device.online ? 1 : 0.72)
    .onClick(() => this.openWorkspacePicker())
  }

  @Builder
  private AgentSection() {
    Column() {
      Row() {
        Text(RemoteI18n.t('sidebar.agents'))
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(MUTED)
        if (this.agentSessions().length > 0) {
          Text(`${this.agentSessions().length}`)
            .fontSize(12)
            .fontColor(SUBTLE)
            .margin({ left: 7 })
        }
        Blank()
        Stack({ alignContent: Alignment.Center }) {
          SymbolGlyph($r('sys.symbol.plus'))
            .fontSize(16)
            .fontColor([MUTED])
        }
        .width(34)
        .height(34)
        .accessibilityText(RemoteI18n.t('sidebar.newAgent'))
        .onClick(() => this.createAgent())
      }
      .width('100%')
      .height(38)
      .padding({ left: this.bodyIndent(), right: 6 })
      .alignItems(VerticalAlign.Center)

      if (this.agentSessions().length === 0) {
        this.EmptyAgents()
      } else {
        ForEach(this.agentSessions(), (item: RemoteSession) => {
          this.AgentRow(item)
        }, (item: RemoteSession): string => `${this.device.deviceId}:${item.id}`)
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
  }

  @Builder
  private AgentRow(item: RemoteSession) {
    Row({ space: 10 }) {
      Stack({ alignContent: Alignment.BottomStart }) {
        SymbolGlyph(this.agentGlyph(item))
          .fontSize(18)
          .fontColor([MUTED])
        if (item.id === this.runningSessionId) {
          Text('')
            .width(7)
            .height(7)
            .backgroundColor(GREEN)
            .borderRadius(4)
        }
      }
      .width(21)
      .height(21)
      Column({ space: 2 }) {
        Text(item.title || RemoteI18n.t('sidebar.untitledAgent'))
          .width('100%')
          .fontSize(15)
          .fontWeight(item.id === this.selectedSessionId ? FontWeight.Medium : FontWeight.Regular)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(this.agentWorkspaceLabel(item))
          .width('100%')
          .fontSize(12)
          .fontColor(SUBTLE)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      if (item.id === this.runningSessionId) {
        Text(RemoteI18n.t('sidebar.agentRunning'))
          .fontSize(11)
          .fontColor(GREEN)
      }
    }
    .width('100%')
    .height(54)
    .padding({ left: this.bodyIndent(), right: 10 })
    .alignItems(VerticalAlign.Center)
    .backgroundColor(item.id === this.selectedSessionId ? SOFT : '#00000000')
    .borderRadius(10)
    .onClick(() => this.onOpenSession(item))
    .gesture(LongPressGesture({ repeat: false }).onAction(() => this.onSessionActions(item)))
  }

  @Builder
  private LoadingRow() {
    Row({ space: 8 }) {
      LoadingProgress().width(14).height(14).color(MUTED)
      Text(RemoteI18n.t('common.loading')).fontSize(13).fontColor(MUTED)
    }
    .width('100%')
    .height(42)
    .padding({ left: this.bodyIndent(), right: 10 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private FailedRow() {
    Row({ space: 8 }) {
      Text(this.device.errorText.length > 0 ?
        this.device.errorText : RemoteI18n.t('sidebar.deviceLoadFailed'))
        .fontSize(13)
        .fontColor(MUTED)
        .layoutWeight(1)
      Text(RemoteI18n.t('common.retry'))
        .fontSize(13)
        .fontColor(INK)
        .onClick(() => this.onRetry())
    }
    .width('100%')
    .height(42)
    .padding({ left: this.bodyIndent(), right: 10 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private EmptyAgents() {
    Text(this.query.trim().length > 0 ?
      RemoteI18n.t('sidebar.noMatchingAgents') : RemoteI18n.t('sidebar.emptyAgents'))
      .fontSize(13)
      .fontColor(MUTED)
      .width('100%')
      .padding({ left: this.bodyIndent(), top: 6, bottom: 10 })
  }

  private openWorkspacePicker(): void {
    this.onChooseWorkspace();
  }

  private createAgent(): void {
    if (this.isActive && this.workspacePath.length > 0 && this.device.online) {
      this.onCreateInWorkspace(this.workspacePath);
      return;
    }
    this.onChooseWorkspace();
  }

  private shouldShowAgents(): boolean {
    if (!this.showHeader) {
      return true;
    }
    return this.device.expanded || this.isActive;
  }

  private bodyIndent(): number {
    return this.showHeader ? 20 : 10;
  }

  private projection(): SessionListProjection {
    const inputs: SessionListInputs = {
      sessions: this.sessions,
      query: this.query,
      sortMode: 'project',
      workspaceName: this.workspaceName,
      workspacePath: this.workspacePath,
      workspaceKind: this.workspaceKind,
      recentWorkspaces: this.recentWorkspaces,
      workspaceFilter: '',
      agentFilter: '',
      statusFilter: '',
      showWorkspaceMetadata: false,
      showUpdatedMetadata: false,
      showStatusMetadata: false
    };
    return this.projectionCache.get(inputs);
  }

  private agentSessions(): RemoteSession[] {
    return SidebarAgentProjectionPolicy.activeAgents(
      this.projection().filtered,
      this.selectedSessionId,
      this.runningSessionId
    );
  }

  private deviceContextLabel(): string {
    if (!this.device.online) {
      return RemoteI18n.t('sidebar.selectWorkspace');
    }
    if (this.isActive && this.workspacePath.length > 0) {
      return this.workspaceName || this.basename(this.workspacePath);
    }
    return RemoteI18n.t('sidebar.selectWorkspace');
  }

  private agentWorkspaceLabel(item: RemoteSession): string {
    if (item.workspaceName && item.workspaceName.length > 0) {
      return item.workspaceName;
    }
    const path = item.workspacePath || '';
    if (path.length > 0) {
      const workspace = this.recentWorkspaces.find((entry: RecentWorkspaceEntry): boolean => entry.path === path);
      return workspace ? workspace.name : this.basename(path);
    }
    return RemoteI18n.t('sidebar.agentChatWorkspace');
  }

  private agentGlyph(item: RemoteSession): Resource {
    const agentType = (item.agentType ? item.agentType : '').toLowerCase();
    if (agentType === 'code') {
      return $r('sys.symbol.code_square');
    }
    if (agentType === 'claw' || agentType === 'assistant' || agentType === 'chat') {
      return $r('sys.symbol.message');
    }
    return $r('sys.symbol.doc_text');
  }

  private basename(path: string): string {
    const normalized = path.replace(/\/+$/, '');
    const index = normalized.lastIndexOf('/');
    return index >= 0 ? normalized.substring(index + 1) : normalized;
  }
}
