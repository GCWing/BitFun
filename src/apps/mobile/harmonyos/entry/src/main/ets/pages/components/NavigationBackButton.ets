import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, LINE } from './Theme';
import { TemplateIcon } from './TemplateIcon';

/** App-wide floating page return affordance. */
@ComponentV2
export struct NavigationBackButton {
  @Param controlSize: number = 48;
  @Param isEnabled: boolean = true;
  @Event onBack: () => void = () => {};

  build() {
    Stack({ alignContent: Alignment.Center }) {
      TemplateIcon({
        src: $r('app.media.remote_ref_back'),
        iconWidth: 15,
        iconHeight: 23
      })
    }
    .width(this.controlSize)
    .height(this.controlSize)
    .backgroundColor(CARD)
    .border({ width: 1, color: LINE })
    .borderRadius(this.controlSize / 2)
    .shadow({ radius: 12, color: LINE, offsetY: 4 })
    .opacity(this.isEnabled ? 1 : 0.4)
    .enabled(this.isEnabled)
    .accessibilityText(RemoteI18n.t('common.back'))
    .onClick(() => {
      this.onBack();
    })
  }
}
