import { INK, MUTED } from './Theme';
import { TemplateIcon } from './TemplateIcon';

/** Desktop-provided device artwork normalized as a theme-tinted template icon. */
@ComponentV2
export struct SidebarDeviceIcon {
  @Param deviceName: string = '';
  @Param iconSize: number = 24;
  @Param tint: ResourceColor = INK;

  build() {
    Stack({ alignContent: Alignment.Center }) {
      TemplateIcon({
        src: this.deviceResource(),
        iconWidth: this.deviceIconWidth(),
        iconHeight: this.iconSize,
        tint: this.tint
      })
        .translate({
          x: this.deviceOpticalOffsetX(),
          y: this.deviceOpticalOffsetY()
        })
    }
    .width(this.iconSize + 8)
    .height(this.iconSize + 4)
  }

  private deviceResource(): Resource {
    const normalizedName = this.deviceName.trim().toLowerCase();
    if (normalizedName.includes('macbook') || normalizedName.includes('laptop') ||
      normalizedName.includes('notebook')) {
      return $r('app.media.sidebar_device_laptop');
    }
    if (normalizedName.includes('server') || normalizedName.includes('ecs') ||
      normalizedName.includes('host')) {
      return $r('app.media.sidebar_device_server');
    }
    return $r('app.media.sidebar_device_desktop');
  }

  private deviceIconWidth(): number {
    const normalizedName = this.deviceName.trim().toLowerCase();
    if (normalizedName.includes('server') || normalizedName.includes('ecs') ||
      normalizedName.includes('host')) {
      return this.iconSize + 8;
    }
    return this.iconSize + 4;
  }

  // The laptop and server artwork has asymmetric transparent padding. Keep the
  // row's layout slot stable and compensate only the visible glyph's center.
  private deviceOpticalOffsetX(): number {
    const normalizedName = this.deviceName.trim().toLowerCase();
    if (normalizedName.includes('server') || normalizedName.includes('ecs') ||
      normalizedName.includes('host')) {
      return -3;
    }
    return 0;
  }

  private deviceOpticalOffsetY(): number {
    const normalizedName = this.deviceName.trim().toLowerCase();
    if (normalizedName.includes('macbook') || normalizedName.includes('laptop') ||
      normalizedName.includes('notebook') || normalizedName.includes('server') ||
      normalizedName.includes('ecs') || normalizedName.includes('host')) {
      return 2;
    }
    return 0;
  }
}

/** HarmonyOS disclosure symbol kept in a stable slot by the caller. */
@ComponentV2
export struct SidebarChevronIcon {
  @Param expanded: boolean = false;
  @Param iconSize: number = 14;
  @Param tint: ResourceColor = MUTED;

  build() {
    SymbolGlyph(this.expanded ?
      $r('sys.symbol.chevron_down') : $r('sys.symbol.chevron_right'))
      .fontSize(this.iconSize)
      .fontColor([this.tint])
  }
}
