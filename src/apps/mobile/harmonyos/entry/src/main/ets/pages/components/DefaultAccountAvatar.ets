import { MUTED, SOFT } from './Theme';

@ComponentV2
export struct DefaultAccountAvatar {
  @Param avatarSize: number = 34;
  @Param avatarUrl: string = '';
  @Local failedUrl: string = '';

  build() {
    Stack({ alignContent: Alignment.Center }) {
      SymbolGlyph($r('sys.symbol.person'))
        .fontSize(this.avatarSize * 0.52)
        .fontColor([MUTED])
      if (this.avatarUrl.length > 0 && this.failedUrl !== this.avatarUrl) {
        Image(this.avatarUrl)
          .width(this.avatarSize)
          .height(this.avatarSize)
          .objectFit(ImageFit.Cover)
          .borderRadius(this.avatarSize / 2)
          .onError(() => { this.failedUrl = this.avatarUrl; })
      }
    }
    .width(this.avatarSize)
    .height(this.avatarSize)
    .backgroundColor(SOFT)
    .borderRadius(this.avatarSize / 2)
  }
}
