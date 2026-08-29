import { SHEET_HEADER_HEIGHT, SHEET_HEADER_SIDE_PADDING } from './SheetLayout';
import { SheetCloseButton } from './SheetCloseButton';

/** Standard close-only header for a modal sheet whose title lives in its body. */
@ComponentV2
export struct SheetCloseHeader {
  @Event onClose: () => void = () => {};

  build() {
    Row() {
      Blank()
      SheetCloseButton({
        onClose: this.onClose
      })
    }
    .width('100%')
    .height(SHEET_HEADER_HEIGHT)
    .padding({ left: SHEET_HEADER_SIDE_PADDING, right: SHEET_HEADER_SIDE_PADDING })
    .alignItems(VerticalAlign.Center)
  }
}
