import { RemoteI18n } from '../../i18n/RemoteI18n';
import {
  CARD,
  INK,
  LINE,
  MUTED,
  SHADOW_SUBTLE,
  TRANSPARENT,
} from './Theme';

@ComponentV2
export struct SidebarToggleButton {
  @Param restore: boolean = false;
  @Param controlSize: number = 44;
  @Event onToggle: () => void = () => {};

  build() {
    Stack({ alignContent: Alignment.Center }) {
      Row() {
        Text('')
          .width(6)
          .height(14)
          .backgroundColor(INK)
          .opacity(0.18)
          .borderRadius({ topLeft: 2, bottomLeft: 2 })
        Divider()
          .vertical(true)
          .height(14)
          .color(INK)
          .opacity(0.7)
        Blank()
          .layoutWeight(1)
      }
      .width(20)
      .height(18)
      .padding({ left: 2, right: 2 })
      .alignItems(VerticalAlign.Center)
      .border({ width: 1.5, color: INK })
      .borderRadius(3)
    }
    .width(this.controlSize)
    .height(this.controlSize)
    .backgroundColor(this.restore ? CARD : TRANSPARENT)
    .border({ width: this.restore ? 1 : 0, color: LINE })
    .borderRadius(this.restore ? this.controlSize / 2 : 8)
    .shadow({
      radius: this.restore ? 14 : 0,
      color: this.restore ? SHADOW_SUBTLE : TRANSPARENT,
      offsetY: this.restore ? 5 : 0
    })
    .accessibilityText(RemoteI18n.t(this.restore ? 'sidebar.restore' : 'sidebar.collapse'))
    .onClick(() => {
      this.onToggle();
    })
  }
}
