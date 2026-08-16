import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ACCENT, CARD, INK, LINE, MUTED, PAGE_BG, PRIMARY_ACTION_TEXT, SOFT } from './Theme';
import { CompactMenuButton } from './CompactMenuButton';
import { TemplateIcon } from './TemplateIcon';
import { SidebarToggleButton } from './SidebarToggleButton';

@ComponentV2
export struct ConversationHeader {
  @Param title: string = '';
  @Param subtitle: string = '';
  @Param titleFallback: string = '';
  @Param allowRename: boolean = false;
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
  @Event onRenameSession: (title: string) => void = (_title: string) => {};
  @Local showTitleEditor: boolean = false;
  @Local renameTitle: string = '';

  build() {
    Column() {
      this.HeaderRow()
      if (this.showTitleEditor) {
        this.TitleEditor()
      }
    }
    .width('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  private HeaderRow() {
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

  @Builder
  private TitleBlock() {
    Column({ space: 3 }) {
      Text(this.resolvedTitle())
        .fontSize(this.hasSubtitle() ? 18 : 17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .textAlign(TextAlign.Center)
        .onClick(() => {
          if (!this.allowRename) {
            return;
          }
          this.renameTitle = this.title;
          this.showTitleEditor = true;
        })
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

  @Builder
  private LeadingControl() {
    if (this.showSidebarRestoreButton) {
      SidebarToggleButton({
        restore: true,
        controlSize: 44,
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
        controlSize: 44,
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
        this.showTitleEditor = false;
        this.onOpenActions();
      })
    } else {
      Blank().width(44).height(44)
    }
  }

  @Builder
  private TitleEditor() {
    Row({ space: 8 }) {
      TextInput({ placeholder: RemoteI18n.t('chat.sessionTitle'), text: this.renameTitle })
        .layoutWeight(1)
        .height(42)
        .fontSize(14)
        .backgroundColor(CARD)
        .borderRadius(14)
        .padding({ left: 12, right: 12 })
        .border({ width: 1, color: LINE })
        .defaultFocus(false)
        .onChange((value: string) => {
          this.renameTitle = value;
        })
      Text(RemoteI18n.t('common.save'))
        .width(52)
        .height(42)
        .fontSize(13)
        .fontColor(PRIMARY_ACTION_TEXT)
        .textAlign(TextAlign.Center)
        .backgroundColor(this.renameTitle.trim().length > 0 ? ACCENT : SOFT)
        .borderRadius(14)
        .onClick(() => {
          if (this.renameTitle.trim().length > 0) {
            this.onRenameSession(this.renameTitle.trim());
            this.showTitleEditor = false;
          }
        })
      Text(RemoteI18n.t('common.cancel'))
        .width(52)
        .height(42)
        .fontSize(13)
        .fontColor(INK)
        .textAlign(TextAlign.Center)
        .backgroundColor(SOFT)
        .borderRadius(14)
        .onClick(() => {
          this.showTitleEditor = false;
        })
    }
    .width('100%')
    .padding({ left: 16, right: 16, top: 10, bottom: 8 })
    .backgroundColor(PAGE_BG)
  }

  private resolvedTitle(): string {
    if (this.title.length > 0) {
      return this.title;
    }
    return this.titleFallback;
  }

  private hasSubtitle(): boolean {
    return this.subtitle.length > 0;
  }

  @Builder
  private EmptyBuilder() {
  }
}
