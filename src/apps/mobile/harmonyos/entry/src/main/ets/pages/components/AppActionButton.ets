import { MobileDesignGeometry, MobileDesignTypography } from '../../generated/MobileDesignTokens';
import {
  CARD,
  CONTENT_ON_ACTION,
  INK,
  LINE,
  PRIMARY_ACTION,
  SHADOW_FAINT,
  SHADOW_SUBTLE,
  SOFT,
  STATUS_DANGER
} from './Theme';

/** Visual intent for a product action. Layout may still choose its own width and height. */
export enum AppActionStyle {
  Primary = 'primary',
  Secondary = 'secondary',
  Quiet = 'quiet',
  Destructive = 'destructive'
}

/** Shared action appearance without forcing every workflow into one fixed geometry. */
@ComponentV2
export struct AppActionButton {
  @Param label: string = '';
  @Param style: AppActionStyle = AppActionStyle.Secondary;
  @Param actionHeight: number = MobileDesignGeometry.sheetActionHeight;
  @Param isEnabled: boolean = true;
  @Event onAction: () => void = () => {};

  build() {
    Button(this.label)
      .width('100%')
      .height(this.actionHeight)
      .fontSize(MobileDesignTypography.labelLarge.size)
      .fontWeight(FontWeight.Medium)
      .fontColor(this.isFilled() ? CONTENT_ON_ACTION : INK)
      .backgroundColor(this.actionBackgroundColor())
      .border({ width: this.style === AppActionStyle.Secondary ? 1 : 0, color: LINE })
      .borderRadius(this.actionHeight / 2)
      .shadow({
        radius: this.hasElevation() ? 14 : 0,
        color: this.isFilled() ? SHADOW_SUBTLE : SHADOW_FAINT,
        offsetY: this.hasElevation() ? 5 : 0
      })
      .opacity(this.isEnabled ? 1 : 0.32)
      .enabled(this.isEnabled)
      .onClick(() => {
        this.onAction();
      })
  }

  private isFilled(): boolean {
    return this.style === AppActionStyle.Primary || this.style === AppActionStyle.Destructive;
  }

  private hasElevation(): boolean {
    return this.isEnabled && this.style !== AppActionStyle.Quiet;
  }

  private actionBackgroundColor(): ResourceColor {
    if (this.style === AppActionStyle.Primary) return PRIMARY_ACTION;
    if (this.style === AppActionStyle.Destructive) return STATUS_DANGER;
    if (this.style === AppActionStyle.Quiet) return SOFT;
    return CARD;
  }
}
