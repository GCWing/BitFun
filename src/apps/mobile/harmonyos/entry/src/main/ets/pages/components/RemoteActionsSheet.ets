import { AssistantEntry, RecentWorkspaceEntry } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, GREEN, INK, LINE, MUTED, RED, SUBTLE } from './Theme';

@ComponentV2
export struct RemoteActionsSheet {
  @Param desktopName: string = 'MacBook Pro';
  @Param workspaceName: string = 'BitFun';
  @Param workspacePath: string = '';
  @Param assistantId: string = '';
  @Param connectionState: string = 'connected';
  @Param isBusy: boolean = false;
  @Param sortMode: string = 'project';
  @Param recentWorkspaces: RecentWorkspaceEntry[] = [];
  @Param assistants: AssistantEntry[] = [];
  @Param showWorkspacePicker: boolean = false;
  @Param showAssistantPicker: boolean = false;
  @Local selectedSortMode: string = 'project';
  @Event onClose: () => void = () => {};
  @Event onDismiss: () => void = () => {};
  @Event onRefresh: () => void = () => {};
  @Event onShowWorkspaces: () => void = () => {};
  @Event onShowAssistants: () => void = () => {};
  @Event onSelectWorkspace: (path: string) => void = (_path: string) => {};
  @Event onSelectAssistant: (path: string) => void = (_path: string) => {};
  @Event onCancelWorkspacePicker: () => void = () => {};
  @Event onCancelAssistantPicker: () => void = () => {};
  @Event onReconnect: () => void = () => {};
  @Event onDisconnect: () => void = () => {};
  @Event onClearPairing: () => void = () => {};
  @Event onSortModeChange: (mode: string) => void = (_mode: string) => {};
  @Event onAddConnection: () => void = () => {};
  @Event onOpenSettings: () => void = () => {};

  aboutToAppear(): void {
    this.selectedSortMode = this.sortMode;
  }

  build() {
    Column() {
      Scroll() {
        Column({ space: 0 }) {
          if (this.showWorkspacePicker) {
            this.WorkspacePicker()
          } else if (this.showAssistantPicker) {
            this.AssistantPicker()
          } else {
            this.RemoteActionRows()
          }
        }
        .width('100%')
        .padding({ left: 16, right: 16, top: 14, bottom: 14 })
      }
      .layoutWeight(1)
      .width('100%')
      .scrollBar(BarState.Off)
    }
    .width('100%')
    .height('100%')
    .backgroundColor(CARD)
    .border({ width: 1, color: '#0D000000' })
    .borderRadius(20)
    .shadow({ radius: 20, color: '#1A000000', offsetY: 8 })
  }

  @Builder
  private RemoteActionRows() {
    Column({ space: 0 }) {
      this.SectionTitle(RemoteI18n.t('remote.menu.organize'))
      this.IconRow('remote_actions_folder', RemoteI18n.t('remote.menu.byProject'), 'project', () => {
        this.selectSortMode('project');
      })
      this.IconRow('remote_actions_clock', RemoteI18n.t('remote.menu.byTime'), 'time', () => {
        this.selectSortMode('time');
      })
      this.IconRow('remote_actions_chat', RemoteI18n.t('remote.menu.chatFirst'), 'chat', () => {
        this.selectSortMode('chat');
      })
      Divider().strokeWidth(1).color(LINE).margin({ top: 8, bottom: 8 })
      this.SectionTitle(RemoteI18n.t('remote.menu.manage'))
      this.IconRow('remote_actions_cloud', RemoteI18n.t('remote.menu.cloudTasks'), '', () => {
        this.getUIContext().getPromptAction().showToast({
          message: RemoteI18n.t('remote.menu.comingSoon'),
          duration: 1800
        });
      })
      this.IconRow('remote_actions_link', RemoteI18n.t('remote.menu.addConnection'), '', () => {
        this.onAddConnection();
        this.onDismiss();
      })
      this.IconRow('remote_actions_settings', RemoteI18n.t('remote.menu.settings'), '', () => {
        this.onDismiss();
        this.onOpenSettings();
      })
    }
    .width('100%')
    .backgroundColor('#00000000')
  }

  @Builder
  private SectionTitle(title: string) {
    Text(title)
      .fontSize(13)
      .fontWeight(FontWeight.Medium)
      .fontColor('#8E8E93')
      .width('100%')
      .height(28)
      .padding({ left: 8 })
      .textAlign(TextAlign.Start)
  }

  @Builder
  private IconRow(icon: string, label: string, sortMode: string, action: () => void) {
    Row({ space: 10 }) {
      if (sortMode.length > 0 && this.selectedSortMode === sortMode) {
        Image($r('app.media.remote_actions_check'))
          .width(20)
          .height(20)
          .objectFit(ImageFit.Contain)
      } else {
        Blank().width(20)
      }
      this.ActionIcon(icon)
      Text(label)
        .fontSize(15)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(48)
    .padding({ left: 8, right: 8 })
    .borderRadius(10)
    .backgroundColor(sortMode.length > 0 && this.selectedSortMode === sortMode ? '#F3F3F3' : '#00000000')
    .onClick(action)
  }

  @Builder
  private ActionIcon(icon: string) {
    if (icon === 'remote_actions_folder') {
      Image($r('app.media.remote_actions_folder')).width(23).height(23).objectFit(ImageFit.Contain)
    } else if (icon === 'remote_actions_clock') {
      Image($r('app.media.remote_actions_clock')).width(23).height(23).objectFit(ImageFit.Contain)
    } else if (icon === 'remote_actions_chat') {
      Image($r('app.media.remote_actions_chat')).width(23).height(23).objectFit(ImageFit.Contain)
    } else if (icon === 'remote_actions_cloud') {
      Image($r('app.media.remote_actions_cloud')).width(23).height(23).objectFit(ImageFit.Contain)
    } else if (icon === 'remote_actions_link') {
      Image($r('app.media.remote_actions_link')).width(23).height(23).objectFit(ImageFit.Contain)
    } else {
      Image($r('app.media.remote_actions_settings')).width(23).height(23).objectFit(ImageFit.Contain)
    }
  }

  @Builder
  private InfoRow(label: string, value: string) {
    Row({ space: 14 }) {
      Text(label)
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Text(value)
        .fontSize(14)
        .fontColor(MUTED)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .constraintSize({ maxWidth: 180 })
    }
    .width('100%')
    .height(54)
    .padding({ left: 18, right: 18 })
    .border({ width: { bottom: 1 }, color: LINE })
  }

  @Builder
  private ActionRow(label: string, action: () => void, destructive: boolean = false) {
    Row() {
      Text(label)
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(destructive ? RED : INK)
      Blank()
      Text('›')
        .fontSize(21)
        .fontColor(SUBTLE)
    }
    .width('100%')
    .height(54)
    .padding({ left: 18, right: 18 })
    .border({ width: { bottom: 1 }, color: LINE })
    .onClick(action)
  }

  @Builder
  private WorkspacePicker() {
    Column({ space: 0 }) {
      this.PickerHeader(RemoteI18n.t('home.recentWorkspaces'), () => {
        this.onCancelWorkspacePicker();
      })
      if (this.recentWorkspaces.length === 0) {
        this.PickerEmpty(this.isBusy ? RemoteI18n.t('common.loading') : RemoteI18n.t('home.noRecentWorkspaces'))
      } else {
        ForEach(this.recentWorkspaces, (item: RecentWorkspaceEntry) => {
          this.PickerRow(item.name || this.basename(item.path), item.path, item.path === this.workspacePath, () => {
            if (item.path !== this.workspacePath) {
              this.onSelectWorkspace(item.path);
            }
            this.onDismiss();
          })
        }, (item: RecentWorkspaceEntry) => item.path)
      }
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(8)
  }

  @Builder
  private AssistantPicker() {
    Column({ space: 0 }) {
      this.PickerHeader(RemoteI18n.t('home.assistantWorkspaces'), () => {
        this.onCancelAssistantPicker();
      })
      if (this.assistants.length === 0) {
        this.PickerEmpty(this.isBusy ? RemoteI18n.t('common.loading') : RemoteI18n.t('home.noAssistants'))
      } else {
        ForEach(this.assistants, (item: AssistantEntry) => {
          this.PickerRow(item.name || this.basename(item.path), item.path, item.assistant_id === this.assistantId, () => {
            this.onSelectAssistant(item.path);
            this.onDismiss();
          })
        }, (item: AssistantEntry) => item.path)
      }
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(8)
  }

  @Builder
  private PickerHeader(title: string, action: () => void) {
    Row() {
      Text('‹')
        .fontSize(28)
        .fontColor(INK)
      Text(title)
        .fontSize(17)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .margin({ left: 8 })
    }
    .width('100%')
    .height(54)
    .padding({ left: 16, right: 16 })
    .border({ width: { bottom: 1 }, color: LINE })
    .onClick(action)
  }

  @Builder
  private PickerRow(title: string, detail: string, selected: boolean, action: () => void) {
    Row({ space: 12 }) {
      Column({ space: 3 }) {
        Text(title)
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(detail)
          .fontSize(12)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Text(selected ? '✓' : '›')
        .fontSize(selected ? 16 : 20)
        .fontColor(selected ? GREEN : SUBTLE)
    }
    .width('100%')
    .height(62)
    .padding({ left: 18, right: 18 })
    .border({ width: { bottom: 1 }, color: LINE })
    .onClick(action)
  }

  @Builder
  private PickerEmpty(text: string) {
    Text(text)
      .width('100%')
      .padding({ left: 18, right: 18, top: 22, bottom: 22 })
      .fontSize(14)
      .lineHeight(20)
      .fontColor(MUTED)
  }

  private workspaceTitle(): string {
    return this.workspaceName || 'BitFun';
  }

  private assistantTitle(): string {
    return this.assistantId || RemoteI18n.t('remote.noAssistant');
  }

  private basename(path: string): string {
    if (!path) {
      return 'Workspace';
    }
    const parts = path.replace(/\\/g, '/').split('/');
    return parts[parts.length - 1] || 'Workspace';
  }

  private selectSortMode(mode: string): void {
    this.selectedSortMode = mode;
    this.onSortModeChange(mode);
  }
}
