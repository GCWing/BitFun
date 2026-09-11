import { MotionPreference } from '../../services/MotionPreference';
import { BrandMark } from './BrandMark';
import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { PAGE_BG, INK } from './Theme';

/** A brief cold-start brand reveal, independent of account/network readiness. */
@ComponentV2
export struct StartupBrandReveal {
  @Event onFinished: () => void = () => {};
  @Local opacityValue: number = 1;
  @Local markScale: number = 0.94;
  @Local lettersVisible: boolean = false;
  private readonly letters: string[] = ['O', 'p', 'e', 'n', 'B', 'i', 't', 'F', 'u', 'n'];
  private entranceTimer: number = -1;
  private exitTimer: number = -1;
  private active: boolean = false;

  aboutToAppear(): void {
    this.active = true;
    if (MotionPreference.reduceMotion()) {
      this.onFinished();
      return;
    }
    this.entranceTimer = setTimeout(() => {
      if (!this.active) return;
      this.lettersVisible = true;
      this.getUIContext().animateTo({ duration: 360, curve: Curve.EaseOut }, () => {
        this.markScale = 1;
      });
    }, 16);
    this.exitTimer = setTimeout(() => {
      if (!this.active) return;
      this.getUIContext().animateTo({
        duration: 240,
        curve: Curve.EaseOut,
        onFinish: () => { if (this.active) this.onFinished(); }
      }, () => { this.opacityValue = 0; });
    }, 1050);
  }

  aboutToDisappear(): void {
    this.active = false;
    clearTimeout(this.entranceTimer);
    clearTimeout(this.exitTimer);
  }

  build() {
    Stack({ alignContent: Alignment.Center }) {
      Column({ space: 24 }) {
        BrandMark({ markSize: 104 }).scale({ x: this.markScale, y: this.markScale })
        Row() {
          ForEach(this.letters, (letter: string, index: number) => {
            Text(letter)
              .fontSize(MobileDesignTypography.displayLarge.size)
              .fontWeight(FontWeight.Medium).fontColor(INK)
              .opacity(this.lettersVisible ? 1 : 0)
              .translate({ y: this.lettersVisible ? 0 : 8 })
              .animation({ duration: 260, delay: 140 + index * 55, curve: Curve.EaseOut })
          }, (_letter: string, index: number) => `${index}`)
        }
        .height(34).justifyContent(FlexAlign.Center)
      }
      .alignItems(HorizontalAlign.Center)
    }
    .width('100%').height('100%').backgroundColor(PAGE_BG)
    .opacity(this.opacityValue).hitTestBehavior(HitTestMode.Block)
    .accessibilityLevel('no-hide-descendants')
  }
}
