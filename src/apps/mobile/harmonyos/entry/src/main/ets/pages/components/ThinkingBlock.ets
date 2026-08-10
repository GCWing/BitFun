import { MUTED } from './Theme';

@Component
export struct ThinkingBlock {
  @Prop text: string = '';
  @Prop status: string = '';
  @Prop keepExpandedWhenDone: boolean = false;
  @Prop streaming: boolean = false;
  @Prop streamKey: string = '';
  onCopyText: (text: string) => void = (_text: string) => {};
  @State dotPhase: number = 0;
  private dotTimerId: number = 0;

  aboutToAppear(): void {
    if (this.isRunning()) {
      this.dotTimerId = setInterval(() => {
        this.dotPhase = (this.dotPhase + 1) % 3;
      }, 360);
    }
  }

  aboutToDisappear(): void {
    if (this.dotTimerId !== 0) {
      clearInterval(this.dotTimerId);
      this.dotTimerId = 0;
    }
  }

  build() {
    if (this.isRunning()) {
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
      .height(24)
      .padding({ left: 2 })
    }
  }

  private dotOpacity(index: number): number {
    return this.dotPhase === index ? 1.0 : 0.34;
  }

  private isRunning(): boolean {
    const normalized = (this.status || '').toLowerCase();
    return normalized === 'active' || normalized === 'running';
  }
}
