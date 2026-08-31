import { RemoteI18n } from '../../i18n/RemoteI18n';
import { TemplateIcon } from './TemplateIcon';
import { CARD, SHADOW_SUBTLE } from './Theme';

const MENU_SURFACE_SIZE: number = 42;

@ComponentV2
export struct CompactMenuButton {
  @Param controlSize: number = 48;
  @Event onOpen: () => void = () => {};
  @Local pressed: boolean = false;

  build() {
    Stack({ alignContent: Alignment.Center }) {
      Stack({ alignContent: Alignment.Center }) {
        TemplateIcon({
          src: $r('app.media.gpt_home_menu_glyph'),
          iconWidth: 23,
          iconHeight: 16
        })
      }
      .width(MENU_SURFACE_SIZE)
      .height(MENU_SURFACE_SIZE)
      .backgroundColor(CARD)
      .borderRadius(MENU_SURFACE_SIZE / 2)
      .shadow({ radius: 30, color: SHADOW_SUBTLE, offsetY: 7 })
      .scale({ x: this.pressed ? 0.96 : 1, y: this.pressed ? 0.96 : 1 })
      .animation({ duration: 120, curve: Curve.EaseOut })
      .hitTestBehavior(HitTestMode.None)
    }
    .width(this.controlSize)
    .height(this.controlSize)
    .accessibilityText(RemoteI18n.t('sidebar.more'))
    .onTouch((event: TouchEvent) => {
      if (event.type === TouchType.Down) {
        this.pressed = true;
      } else if (event.type === TouchType.Up || event.type === TouchType.Cancel) {
        this.pressed = false;
      }
    })
    .onClick(() => {
      this.pressed = false;
      this.onOpen();
    })
  }
}
