import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ACCENT, CARD, INK, SUBTLE } from './Theme';

@ComponentV2
export struct RemoteBottomBar {
  @Param query: string = '';
  @Param isBusy: boolean = false;
  @Event onQueryChange: (value: string) => void = (_value: string) => {};
  @Event onSearch: () => void = () => {};
  @Event onCreate: () => void = () => {};

  build() {
    Row({ space: 12 }) {
      Row({ space: 8 }) {
        Image($r('app.media.remote_ref_search_reference'))
          .width(24)
          .height(24)
          .objectFit(ImageFit.Contain)
        TextInput({ placeholder: RemoteI18n.t('remote.searchChats'), text: this.query })
          .layoutWeight(1)
          .height(48)
          .fontSize(16)
          .fontColor(INK)
          .placeholderColor(SUBTLE)
          .padding({ left: 0, right: 4 })
          .backgroundColor('#00000000')
          .onChange((value: string) => {
            this.onQueryChange(value);
          })
      }
      .layoutWeight(1)
      .height(48)
      .padding({ left: 6, right: 10 })
      .backgroundColor(CARD)
      .borderRadius(24)
      .shadow({ radius: 20, color: '#12000000', offsetY: 7 })
      .onClick(() => {
        if (this.query.trim().length > 0) {
          this.onSearch();
        }
      })

      Button() {
        Row({ space: 9 }) {
          Image($r('app.media.remote_ref_new_chat'))
            .width(24)
            .height(24)
            .objectFit(ImageFit.Contain)
          Text(RemoteI18n.t('remote.newChat'))
            .fontSize(16)
            .fontWeight(FontWeight.Medium)
            .fontColor(CARD)
        }
      }
      .width(118)
      .height(48)
      .padding(0)
      .backgroundColor(ACCENT)
      .borderRadius(24)
      .shadow({ radius: 20, color: '#18000000', offsetY: 7 })
      .enabled(!this.isBusy)
      .onClick(() => {
        this.onCreate();
      })
    }
    .width('91%')
    .alignSelf(ItemAlign.Center)
    .alignItems(VerticalAlign.Center)
    .translate({ y: 15 })
  }
}
