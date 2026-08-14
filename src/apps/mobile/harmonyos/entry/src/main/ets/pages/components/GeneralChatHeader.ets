import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, MUTED, PAGE_BG } from './Theme';
import { CompactMenuButton } from './CompactMenuButton';
import { TemplateIcon } from './TemplateIcon';
import { SidebarToggleButton } from './SidebarToggleButton';

@ComponentV2
export struct GeneralChatHeader {
  @Param title: string = '';
  /** Secondary context line. Empty keeps the single-line header. */
  @Param subtitle: string = '';
  @Param showActions: boolean = false;
  @Param showSidebarButton: boolean = true;
  @Param showBackButton: boolean = false;
  @Param showSidebarRestoreButton: boolean = false;
  @Param showActionsMenu: boolean = false;
  @BuilderParam actionsMenu: () => void = this.EmptyBuilder;
  @Event onOpenSidebar: () => void = () => {};
  @Event onRestoreSidebar: () => void = () => {};
  @Event onBack: () => void = () => {};
  @Event onOpenActions: () => void = () => {};
  @Event onActionsMenuStateChange: (visible: boolean) => void = (_visible: boolean) => {};

  build() {
    Row({ space: 8 }) {
      this.LeadingControl()
      this.TitleBlock()
      this.TrailingControl()
    }
    .width('100%')
    .height(this.hasSubtitle() ? 76 : 64)
    .alignItems(VerticalAlign.Center)
    .padding({ left: 16, right: 16, top: 8, bottom: 8 })
    .backgroundColor(PAGE_BG)
  }

  /** Mirrors the conversation header: title above a muted context line. */
  @Builder
  private TitleBlock() {
    Column({ space: 3 }) {
      Text(this.title || 'BitFun')
        .fontSize(this.hasSubtitle() ? 18 : 17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .textAlign(TextAlign.Center)
      if (this.hasSubtitle()) {
        Text(this.subtitle)
          .fontSize(14)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .textAlign(TextAlign.Center)
      }
    }
    .layoutWeight(1)
    .alignItems(HorizontalAlign.Center)
  }

  private hasSubtitle(): boolean {
    return this.subtitle.length > 0;
  }

  @Builder
  private LeadingControl() {
    if (this.showSidebarRestoreButton) {
      SidebarToggleButton({
        restore: true,
        controlSize: 48,
        onToggle: this.onRestoreSidebar
      })
    } else if (this.showBackButton) {
      Stack({ alignContent: Alignment.Center }) {
        TemplateIcon({
          src: $r('app.media.remote_ref_back'),
          iconWidth: 15,
          iconHeight: 23
        })
      }
      .width(44)
      .height(44)
      .backgroundColor(CARD)
      .border({ width: 1, color: LINE })
      .borderRadius(22)
      .shadow({ radius: 10, color: LINE, offsetY: 3 })
      .accessibilityText(RemoteI18n.t('common.back'))
      .onClick(() => {
        this.onBack();
      })
    } else if (this.showSidebarButton) {
      CompactMenuButton({
        onOpen: () => {
          this.onOpenSidebar();
        }
      })
    } else {
      Blank().width(44).height(44)
    }
  }

  @Builder
  private TrailingControl() {
    if (this.showActions) {
      Stack({ alignContent: Alignment.Center }) {
        TemplateIcon({
          src: $r('app.media.remote_ref_more'),
          iconWidth: 23,
          iconHeight: 7
        })
      }
      .width(44)
      .height(44)
      .backgroundColor(CARD)
      .border({ width: 1, color: LINE })
      .borderRadius(22)
      .shadow({ radius: 10, color: LINE, offsetY: 3 })
      .accessibilityText(RemoteI18n.t('sidebar.more'))
      .bindPopup(this.showActionsMenu, {
        builder: () => {
          this.actionsMenu();
        },
        placement: Placement.BottomRight,
        popupColor: '#00000000',
        enableArrow: false,
        autoCancel: true,
        mask: false,
        targetSpace: 8,
        onStateChange: (event) => {
          this.onActionsMenuStateChange(event.isVisible);
        }
      })
      .onClick(() => {
        this.onOpenActions();
      })
    } else {
      Blank().width(44).height(44)
    }
  }

  @Builder
  private EmptyBuilder() {
  }
}
