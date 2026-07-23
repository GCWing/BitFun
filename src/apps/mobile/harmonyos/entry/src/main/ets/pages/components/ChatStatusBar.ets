import { RemoteI18n } from '../../i18n/RemoteI18n';
import { INK, LINE, MUTED, PAGE_BG } from './Theme';

@Component
export struct ChatStatusBar {
  @Prop title: string = '';
  @Prop detail: string = '';
  @Prop color: string = MUTED;
  @Prop canStop: boolean = false;
  onStop: () => void = () => {};

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
          .fontSize(13)
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
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .backgroundColor('#F0EFEC')
          .borderRadius(17)
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
