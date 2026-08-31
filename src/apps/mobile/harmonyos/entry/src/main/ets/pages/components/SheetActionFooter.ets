import {
  SHEET_ACTION_HEIGHT,
  SHEET_CONTENT_MAX_WIDTH,
  SHEET_FOOTER_BOTTOM_PADDING,
  SHEET_FOOTER_TOP_PADDING,
  SHEET_HORIZONTAL_PADDING
} from './SheetLayout';
import { AppActionButton, AppActionStyle } from './AppActionButton';

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
        AppActionButton({
          label: this.label,
          style: this.primary ? AppActionStyle.Primary : AppActionStyle.Secondary,
          actionHeight: SHEET_ACTION_HEIGHT,
          isEnabled: this.isEnabled,
          onAction: this.onAction
        })
        .layoutWeight(1)
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
