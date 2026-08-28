import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { INK, LINE, MUTED, PAGE_BG, SOFT } from './Theme';

@ComponentV2
export struct ChatStatusBar {
  @Param title: string = '';
  @Param detail: string = '';
  @Param color: ResourceColor = MUTED;
  @Param canStop: boolean = false;
  @Event onStop: () => void = () => {};

  build() {
    Row({ space: 10 }) {
      if (this.canStop) {
        Blank()
          .width(68)
      }
      Row({ space: 7 }) {
        Text('')
          .width(8)
          .height(8)
          .backgroundColor(this.color)
          .borderRadius(4)
        Text(this.statusLabel())
          .fontSize(MobileDesignTypography.bodySmall.size)
          .fontColor(this.canStop ? MUTED : this.color)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .justifyContent(this.canStop ? FlexAlign.Center : FlexAlign.Start)
      if (this.canStop) {
        Text(RemoteI18n.t('chat.stop'))
          .width(68)
          .height(34)
          .fontSize(MobileDesignTypography.bodySmall.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .backgroundColor(SOFT)
          .borderRadius(17)
          .accessibilityText(RemoteI18n.t('chat.stop'))
          .onClick(() => {
            this.onStop();
          })
      }
    }
    .width('100%')
    .height(48)
    .padding({ left: 24, right: 24 })
    .backgroundColor(PAGE_BG)
    .border({ width: { top: 1, bottom: 1 }, color: LINE })
  }

  private statusLabel(): string {
    if (this.detail.length > 0) {
      return `${this.title} · ${this.detail}`;
    }
    return this.title;
  }
}
