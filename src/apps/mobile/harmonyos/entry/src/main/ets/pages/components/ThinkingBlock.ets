import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ThinkingPresentationPolicy } from '../policy/ThinkingPresentationPolicy';
import { MUTED } from './Theme';

@ComponentV2
export struct ThinkingBlock {
  @Param text: string = '';
  @Param status: string = '';
  @Param keepExpandedWhenDone: boolean = false;
  @Param streaming: boolean = false;
  @Param streamKey: string = '';
  @Local dotPhase: number = 0;
  @Local expanded: boolean = false;
  @Local userToggled: boolean = false;
  private dotTimerId: number = 0;

  aboutToAppear(): void {
    this.userToggled = false;
    this.expanded = ThinkingPresentationPolicy.defaultExpanded(this.isRunning(), this.keepExpandedWhenDone);
    this.syncDotTimer();
  }

  aboutToDisappear(): void {
    this.stopDotTimer();
  }

  @Monitor('status')
  onStatusChanged(): void {
    this.expanded = ThinkingPresentationPolicy.expandedForStatus(
      this.isRunning(),
      this.keepExpandedWhenDone,
      this.userToggled,
      this.expanded
    );
    if (!this.isRunning()) {
      this.userToggled = false;
    }
    this.syncDotTimer();
  }

  build() {
    if (ThinkingPresentationPolicy.shouldRender(this.status, this.text)) {
      Column({ space: 8 }) {
        this.HeaderRow()
        if (this.expanded && this.hasBody()) {
          Text(this.text)
            .fontSize(14)
            .lineHeight(21)
            .fontColor(MUTED)
            .width('100%')
        }
      }
      .width('100%')
      .padding({ left: 2, right: 2, top: 2, bottom: this.expanded && this.hasBody() ? 8 : 2 })
    }
  }

  @Builder
  HeaderRow() {
    Row({ space: 8 }) {
      Row({ space: 8 }) {
        if (this.isRunning()) {
          this.RunningDots()
        } else if (this.expanded) {
          SymbolGlyph($r('sys.symbol.chevron_down'))
            .fontSize(14)
            .fontColor([MUTED])
            .width(16)
            .height(16)
        } else {
          SymbolGlyph($r('sys.symbol.chevron_right'))
            .fontSize(14)
            .fontColor([MUTED])
            .width(16)
            .height(16)
        }
        Text(this.isRunning() ? RemoteI18n.t('chat.thinkingInProgress') : RemoteI18n.t('chat.thinkingComplete'))
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(MUTED)
          .layoutWeight(1)
      }
      .layoutWeight(1)
      .height(32)
      .alignItems(VerticalAlign.Center)
      .onClick(() => {
        if (!ThinkingPresentationPolicy.allowsToggle(this.hasBody())) {
          return;
        }
        this.userToggled = this.isRunning();
        this.expanded = !this.expanded;
      })
      .accessibilityText(this.isRunning() ?
        RemoteI18n.t('chat.thinkingInProgress') :
        RemoteI18n.t('chat.thinkingComplete'))
    }
    .width('100%')
    .height(32)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  RunningDots() {
    Row({ space: 6 }) {
      Text('•')
        .width(6)
        .height(18)
        .fontSize(16)
        .fontColor(MUTED)
        .opacity(this.dotOpacity(0))
        .animation({ duration: 180, curve: Curve.EaseInOut })
      Text('•')
        .width(6)
        .height(18)
        .fontSize(16)
        .fontColor(MUTED)
        .opacity(this.dotOpacity(1))
        .animation({ duration: 180, curve: Curve.EaseInOut })
      Text('•')
        .width(6)
        .height(18)
        .fontSize(16)
        .fontColor(MUTED)
        .opacity(this.dotOpacity(2))
        .animation({ duration: 180, curve: Curve.EaseInOut })
    }
    .width(22)
    .height(18)
  }

  private dotOpacity(index: number): number {
    return this.dotPhase === index ? 1.0 : 0.34;
  }

  private isRunning(): boolean {
    return ThinkingPresentationPolicy.isRunning(this.status);
  }

  private hasBody(): boolean {
    return this.text.trim().length > 0;
  }

  private syncDotTimer(): void {
    if (this.isRunning()) {
      if (this.dotTimerId === 0) {
        this.dotTimerId = setInterval(() => {
          this.dotPhase = (this.dotPhase + 1) % 3;
        }, 360);
      }
      return;
    }
    this.stopDotTimer();
  }

  private stopDotTimer(): void {
    if (this.dotTimerId !== 0) {
      clearInterval(this.dotTimerId);
      this.dotTimerId = 0;
    }
  }
}
