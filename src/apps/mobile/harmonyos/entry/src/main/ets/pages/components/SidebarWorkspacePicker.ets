import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RecentWorkspaceEntry } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, MUTED, PAGE_BG, SOFT } from './Theme';

@ComponentV2
export struct SidebarWorkspacePicker {
  @Param deviceName: string = '';
  @Param workspaces: RecentWorkspaceEntry[] = [];
  @Param selectedWorkspacePath: string = '';
  @Param loading: boolean = false;
  @Event onClose: () => void = () => {};
  @Event onSelectWorkspace: (path: string) => void = (_path: string) => {};

  build() {
    Column() {
      this.Header()
      Divider().color(LINE)
      if (this.loading && this.workspaces.length === 0) {
        this.LoadingState()
      } else if (this.workspaces.length === 0) {
        this.EmptyState()
      } else {
        Scroll() {
          Column({ space: 4 }) {
            ForEach(this.workspaces, (workspace: RecentWorkspaceEntry) => {
              this.WorkspaceRow(workspace)
            }, (workspace: RecentWorkspaceEntry): string => workspace.path)
          }
          .width('100%')
          .padding({ top: 10, bottom: 18 })
        }
        .width('100%')
        .layoutWeight(1)
        .scrollBar(BarState.Off)
      }
    }
    .width('100%')
    .height('100%')
    .padding({ left: 18, right: 18, top: 10 })
    .backgroundColor(PAGE_BG)
  }

  @Builder
  private Header() {
    Row({ space: 12 }) {
      Column({ space: 2 }) {
        Text(RemoteI18n.t('sidebar.selectWorkspace'))
          .width('100%')
          .fontSize(MobileDesignTypography.headlineMedium.size)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
        Text(this.deviceName)
          .width('100%')
          .fontSize(MobileDesignTypography.bodySmall.size)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.xmark'))
          .fontSize(17)
          .fontColor([MUTED])
      }
      .width(40)
      .height(40)
      .borderRadius(20)
      .backgroundColor(SOFT)
      .accessibilityText(RemoteI18n.t('common.close'))
      .onClick(() => this.onClose())
    }
    .width('100%')
    .height(66)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private WorkspaceRow(workspace: RecentWorkspaceEntry) {
    Row({ space: 12 }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.folder'))
          .fontSize(20)
          .fontColor([INK])
      }
      .width(34)
      .height(34)
      .borderRadius(10)
      .backgroundColor(CARD)
      Column({ space: 3 }) {
        Text(workspace.name || this.basename(workspace.path))
          .width('100%')
          .fontSize(MobileDesignTypography.bodyLarge.size)
          .fontWeight(workspace.path === this.selectedWorkspacePath ? FontWeight.Medium : FontWeight.Regular)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(workspace.path)
          .width('100%')
          .fontSize(MobileDesignTypography.labelSmall.size)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph(workspace.path === this.selectedWorkspacePath ?
          $r('sys.symbol.checkmark') : $r('sys.symbol.chevron_right'))
          .fontSize(workspace.path === this.selectedWorkspacePath ? 17 : 15)
          .fontColor([workspace.path === this.selectedWorkspacePath ? INK : MUTED])
      }
      .width(40)
      .height(40)
    }
    .width('100%')
    .height(68)
    .padding({ left: 10, right: 8 })
    .borderRadius(14)
    .backgroundColor(workspace.path === this.selectedWorkspacePath ? SOFT : PAGE_BG)
    .onClick(() => this.onSelectWorkspace(workspace.path))
  }

  @Builder
  private LoadingState() {
    Column({ space: 12 }) {
      LoadingProgress().width(22).height(22).color(MUTED)
      Text(RemoteI18n.t('common.loading')).fontSize(MobileDesignTypography.bodyMedium.size).fontColor(MUTED)
    }
    .width('100%')
    .layoutWeight(1)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  private EmptyState() {
    Column({ space: 8 }) {
      SymbolGlyph($r('sys.symbol.folder')).fontSize(30).fontColor([MUTED])
      Text(RemoteI18n.t('sidebar.emptyWorkspaces'))
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
    }
    .width('100%')
    .layoutWeight(1)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  private basename(path: string): string {
    const normalized = path.replace(/\/+$/, '');
    const index = normalized.lastIndexOf('/');
    return index >= 0 ? normalized.substring(index + 1) : normalized;
  }
}
