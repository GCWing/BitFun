import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { CARD, FILE_LINK, INK, LINE, MUTED, SOFT } from './Theme';

@ComponentV2
export struct FileReferenceCard {
  @Param path: string = '';
  @Param label: string = '';
  @Param status: string = '';
  @Param previewLabel: string = '';
  @Param buttonLabel: string = '';
  @Param disabled: boolean = false;
  @Param selected: boolean = false;
  @Param previewLoading: boolean = false;
  @Event onPreview: (path: string, label: string) => void = (_path: string, _label: string) => {};
  @Event onDownload: (path: string) => void = (_path: string) => {};

  build() {
    Row({ space: 10 }) {
      Row({ space: 10 }) {
        Stack({ alignContent: Alignment.Center }) {
          if (this.previewLoading) {
            LoadingProgress()
              .width(17)
              .height(17)
              .color(FILE_LINK)
          } else {
            Text('▤')
              .fontSize(MobileDesignTypography.bodyLarge.size)
              .fontColor(INK)
          }
        }
        .width(34)
        .height(34)
        .backgroundColor(SOFT)
        .borderRadius(12)
        Column({ space: 3 }) {
          Text(this.label)
            .fontSize(MobileDesignTypography.bodySmall.size)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
          Text(this.status)
            .fontSize(MobileDesignTypography.labelSmall.size)
            .fontColor(FILE_LINK)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .layoutWeight(1)
        .alignItems(HorizontalAlign.Start)
      }
      .layoutWeight(1)
      .height(44)
      .accessibilityText(`${this.previewLabel} ${this.label}`)
      .onClick(() => {
        this.onPreview(this.path, this.label);
      })
      Stack({ alignContent: Alignment.Center }) {
        if (this.disabled) {
          LoadingProgress()
            .width(18)
            .height(18)
            .color(MUTED)
        } else {
          SymbolGlyph($r('sys.symbol.arrow_down_to_line'))
            .fontSize(19)
            .fontColor([INK])
        }
      }
      .width(44)
      .height(44)
      .accessibilityText(this.buttonLabel)
      .opacity(this.disabled ? 0.55 : 1)
      .onClick(() => {
        if (!this.disabled) {
          this.onDownload(this.path);
        }
      })
    }
    .width('100%')
    .padding(12)
    .backgroundColor(this.selected ? SOFT : CARD)
    .borderRadius(14)
    .border({ width: 1, color: this.selected ? FILE_LINK : LINE })
  }
}
