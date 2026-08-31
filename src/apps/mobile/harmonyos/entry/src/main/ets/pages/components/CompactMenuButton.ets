import { RemoteI18n } from '../../i18n/RemoteI18n';
import { TemplateIcon } from './TemplateIcon';
import { TRANSPARENT } from './Theme';

@ComponentV2
export struct CompactMenuButton {
  @Param controlSize: number = 48;
  @Event onOpen: () => void = () => {};

  build() {
    Button() {
      TemplateIcon({
        src: $r('app.media.gpt_home_menu_glyph'),
        iconWidth: 22,
        iconHeight: 14
      })
    }
    .width(this.controlSize)
    .height(this.controlSize)
    .padding(0)
    .backgroundColor(TRANSPARENT)
    .borderRadius(12)
    .stateEffect(true)
    .accessibilityText(RemoteI18n.t('sidebar.more'))
    .onClick(() => {
      this.onOpen();
    })
  }
}
