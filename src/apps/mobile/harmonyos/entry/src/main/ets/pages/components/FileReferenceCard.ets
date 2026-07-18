import { CARD, INK, LINE, MUTED } from './Theme';

@Component
export struct FileReferenceCard {
  @Prop path: string = '';
  @Prop label: string = '';
  @Prop status: string = '';
  @Prop buttonLabel: string = '';
  @Prop disabled: boolean = false;
  onDownload: (path: string) => void = (_path: string) => {};

  build() {
    Row({ space: 10 }) {
      Text('▤')
        .width(34)
        .height(34)
        .fontSize(16)
        .fontColor(INK)
        .textAlign(TextAlign.Center)
        .backgroundColor('#F3F2EE')
        .borderRadius(12)
      Column({ space: 3 }) {
        Text(this.label)
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(this.status)
          .fontSize(11)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Text(this.buttonLabel)
        .fontSize(12)
        .fontColor(this.disabled ? MUTED : INK)
        .padding({ left: 10, right: 10, top: 6, bottom: 6 })
        .backgroundColor('#F0EFEB')
        .borderRadius(14)
        .border({ width: 1, color: LINE })
        .onClick(() => {
          if (!this.disabled) {
            this.onDownload(this.path);
          }
        })
    }
    .width('100%')
    .padding(12)
    .backgroundColor(CARD)
    .borderRadius(14)
    .border({ width: 1, color: LINE })
  }
}
