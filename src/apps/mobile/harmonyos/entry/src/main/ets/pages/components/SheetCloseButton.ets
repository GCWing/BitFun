import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE } from './Theme';

/** Prominent top-right close affordance for modal sheets. */
@ComponentV2
export struct SheetCloseButton {
  @Param touchSize: number = 44;
  @Param visualSize: number = 36;
  @Param isEnabled: boolean = true;
  @Event onClose: () => void = () => {};

  build() {
    Stack({ alignContent: Alignment.Center }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.xmark'))
          .fontSize(18)
          .fontWeight(FontWeight.Medium)
          .fontColor([INK])
      }
      .width(this.visualSize)
      .height(this.visualSize)
      .backgroundColor(CARD)
      .borderRadius(this.visualSize / 2)
      .shadow({ radius: 8, color: LINE, offsetY: 3 })
    }
    .width(this.touchSize)
    .height(this.touchSize)
    .opacity(this.isEnabled ? 1 : 0.4)
    .enabled(this.isEnabled)
    .accessibilityText(RemoteI18n.t('common.close'))
    .onClick(() => {
      this.onClose();
    })
  }
}
