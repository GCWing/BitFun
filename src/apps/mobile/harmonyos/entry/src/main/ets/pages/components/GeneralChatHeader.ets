import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, PAGE_BG } from './Theme';
import { CompactMenuButton } from './CompactMenuButton';
import { SidebarToggleButton } from './SidebarToggleButton';

@ComponentV2
export struct GeneralChatHeader {
  @Param title: string = '';
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

      Text(this.title || 'BitFun')
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .textAlign(TextAlign.Center)

      this.TrailingControl()
    }
    .width('100%')
    .height(64)
    .alignItems(VerticalAlign.Center)
    .padding({ left: 16, right: 16, top: 8, bottom: 8 })
    .backgroundColor(PAGE_BG)
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
        Image($r('app.media.remote_ref_back'))
          .width(15)
          .height(23)
          .objectFit(ImageFit.Contain)
          .renderMode(ImageRenderMode.Template)
          .foregroundColor(INK)
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
        Image($r('app.media.remote_ref_more'))
          .width(23)
          .height(7)
          .objectFit(ImageFit.Contain)
          .renderMode(ImageRenderMode.Template)
          .foregroundColor(INK)
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
