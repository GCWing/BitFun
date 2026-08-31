import { MobileDesignGeometry } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { INK, TRANSPARENT } from './Theme';

/** Quiet top-right close affordance with a full-size touch target. */
@ComponentV2
export struct SheetCloseButton {
  @Param touchSize: number = MobileDesignGeometry.controlTouchSize;
  @Param isEnabled: boolean = true;
  @Event onClose: () => void = () => {};

  build() {
    Button() {
      SymbolGlyph($r('sys.symbol.xmark'))
        .fontSize(18)
        .fontWeight(FontWeight.Medium)
        .fontColor([INK])
    }
    .width(this.touchSize)
    .height(this.touchSize)
    .padding(0)
    .backgroundColor(TRANSPARENT)
    .borderRadius(12)
    .stateEffect(true)
    .opacity(this.isEnabled ? 1 : 0.4)
    .enabled(this.isEnabled)
    .accessibilityText(RemoteI18n.t('common.close'))
    .onClick(() => {
      this.onClose();
    })
  }
}
