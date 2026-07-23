import { CARD, INK, PAGE_BG } from './Theme';

@Component
export struct GeneralChatHeader {
  onOpenSidebar: () => void = () => {};
  @Prop showActions: boolean = false;
  onNewSession: () => void = () => {};
  onOpenActions: () => void = () => {};

  build() {
    Row() {
      Stack({ alignContent: Alignment.Center }) {
        Image($r('app.media.gpt_home_menu_glyph'))
          .width(26)
          .height(26)
          .objectFit(ImageFit.Contain)
      }
      .width(48)
      .height(48)
      .backgroundColor(CARD)
      .borderRadius(24)
      .shadow({ radius: 16, color: '#10000000', offsetY: 7 })
      .onClick(() => {
        this.onOpenSidebar();
      })
      Text('BitFun')
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .height(48)
        .padding({ left: 18, right: 18 })
        .margin({ left: 12 })
      Blank()
      if (this.showActions) {
        Row() {
          Stack({ alignContent: Alignment.Center }) {
            Image($r('app.media.remote_ref_edit'))
              .width(24).height(24).objectFit(ImageFit.Contain)
          }
          .width(44).height(48)
          .onClick(() => this.onNewSession())
          Stack({ alignContent: Alignment.Center }) {
            Image($r('app.media.remote_ref_more'))
              .width(25).height(8).objectFit(ImageFit.Contain)
          }
          .width(44).height(48)
          .onClick(() => this.onOpenActions())
        }
        .width(90).height(48)
        .padding({ left: 1, right: 1 })
        .backgroundColor('#FFFFFF')
        .borderRadius(24)
        .shadow({ radius: 16, color: '#10000000', offsetY: 7 })
      }
    }
    .width('100%')
    .height(52)
    .alignItems(VerticalAlign.Center)
    .padding({ left: 24, right: 24 })
    .backgroundColor(PAGE_BG)
  }
}
