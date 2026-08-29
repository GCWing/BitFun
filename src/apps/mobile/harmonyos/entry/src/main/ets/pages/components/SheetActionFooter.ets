import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import {
  SHEET_CONTENT_MAX_WIDTH,
  SHEET_FOOTER_BOTTOM_PADDING,
  SHEET_FOOTER_TOP_PADDING,
  SHEET_HORIZONTAL_PADDING
} from './SheetLayout';
import { CARD, INK, LINE, PRIMARY_ACTION, PRIMARY_ACTION_TEXT } from './Theme';

/** Standard single-action footer for full-height modal sheets. */
@ComponentV2
export struct SheetActionFooter {
  @Param label: string = '';
  @Param primary: boolean = false;
  @Param isEnabled: boolean = true;
  @Event onAction: () => void = () => {};

  build() {
    Column() {
      Row() {
        Button(this.label)
          .height(54)
          .layoutWeight(1)
          .fontSize(MobileDesignTypography.labelLarge.size)
          .fontWeight(this.primary ? FontWeight.Bold : FontWeight.Medium)
          .fontColor(this.primary ? PRIMARY_ACTION_TEXT : INK)
          .backgroundColor(this.primary ? PRIMARY_ACTION : CARD)
          .border({ width: this.primary ? 0 : 1, color: LINE })
          .borderRadius(27)
          .opacity(this.isEnabled ? 1 : 0.32)
          .enabled(this.isEnabled)
          .onClick(() => {
            this.onAction();
          })
      }
      .width('100%')
      .constraintSize({ maxWidth: SHEET_CONTENT_MAX_WIDTH })
    }
    .width('100%')
    .padding({
      left: SHEET_HORIZONTAL_PADDING,
      right: SHEET_HORIZONTAL_PADDING,
      top: SHEET_FOOTER_TOP_PADDING,
      bottom: SHEET_FOOTER_BOTTOM_PADDING
    })
    .alignItems(HorizontalAlign.Center)
  }
}
