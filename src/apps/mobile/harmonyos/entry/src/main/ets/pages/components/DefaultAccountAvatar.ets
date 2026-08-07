import { MUTED, SOFT } from './Theme';

@Component
export struct DefaultAccountAvatar {
  @Prop avatarSize: number = 34;

  build() {
    Stack({ alignContent: Alignment.Center }) {
      SymbolGlyph($r('sys.symbol.person'))
        .fontSize(this.avatarSize * 0.52)
        .fontColor([MUTED])
    }
    .width(this.avatarSize)
    .height(this.avatarSize)
    .backgroundColor(SOFT)
    .borderRadius(this.avatarSize / 2)
  }
}
