import { INK } from './Theme';

// Monochrome bitmap glyph tinted with a semantic theme color.
//
// The reference assets are black-pixel PNGs with an alpha mask, and ArkUI
// cannot tint those the way iOS template images work: `fillColor` only applies
// to SVG sources, and `renderMode(ImageRenderMode.Template)` is a monochrome
// render that keeps the original luminance, so the black pixels stayed black on
// dark surfaces. Painting the tint over the glyph in an offscreen layer and
// clipping it to the alpha mask with `SRC_IN` keeps the color a plain
// `ResourceColor`, so it still resolves per theme.
@ComponentV2
export struct TemplateIcon {
  @Require @Param src: Resource;
  @Param iconWidth: number = 24;
  @Param iconHeight: number = 24;
  @Param tint: ResourceColor = INK;

  build() {
    Stack() {
      Image(this.src)
        .width(this.iconWidth)
        .height(this.iconHeight)
        .objectFit(ImageFit.Contain)
        .draggable(false)
      Text('')
        .width(this.iconWidth)
        .height(this.iconHeight)
        .backgroundColor(this.tint)
        .blendMode(BlendMode.SRC_IN)
    }
    .width(this.iconWidth)
    .height(this.iconHeight)
    .blendMode(BlendMode.SRC_OVER, BlendApplyType.OFFSCREEN)
  }
}
