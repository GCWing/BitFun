import { RemoteI18n } from '../../i18n/RemoteI18n';
import { TemplateIcon } from './TemplateIcon';
import { CARD } from './Theme';

@ComponentV2
export struct CompactMenuButton {
  @Param controlSize: number = 48;
  @Event onOpen: () => void = () => {};

  build() {
    Stack({ alignContent: Alignment.Center }) {
      TemplateIcon({
        src: $r('app.media.gpt_home_menu_glyph'),
        iconWidth: 22,
        iconHeight: 14
      })
    }
    .width(this.controlSize)
    .height(this.controlSize)
    .backgroundColor(CARD)
    .borderRadius(this.controlSize / 2)
    .shadow({ radius: 16, color: '#12000000', offsetY: 6 })
    .accessibilityText(RemoteI18n.t('sidebar.more'))
    .onClick(() => {
      this.onOpen();
    })
  }
}
