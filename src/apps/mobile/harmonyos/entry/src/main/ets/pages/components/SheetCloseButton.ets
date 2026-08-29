import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE } from './Theme';

/** Prominent top-right close affordance for modal sheets. */
@ComponentV2
export struct SheetCloseButton {
  @Param controlSize: number = 48;
  @Param isEnabled: boolean = true;
  @Event onClose: () => void = () => {};

  build() {
    Stack({ alignContent: Alignment.Center }) {
      SymbolGlyph($r('sys.symbol.xmark'))
        .fontSize(21)
        .fontWeight(FontWeight.Medium)
        .fontColor([INK])
    }
    .width(this.controlSize)
    .height(this.controlSize)
    .backgroundColor(CARD)
    .borderRadius(this.controlSize / 2)
    .shadow({ radius: 12, color: LINE, offsetY: 4 })
    .opacity(this.isEnabled ? 1 : 0.4)
    .enabled(this.isEnabled)
    .accessibilityText(RemoteI18n.t('common.close'))
    .onClick(() => {
      this.onClose();
    })
  }
}
