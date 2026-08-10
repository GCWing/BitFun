import { SOFT } from './Theme';

/** Initial remote-home loading state, matching the rounded-line reference layout. */
@ComponentV2
export struct RemoteSessionLoadingView {
  build() {
    Scroll() {
      Column({ space: 30 }) {
        this.SkeletonBar('28%');
        this.SkeletonBar('58%');
        this.SkeletonBar('90%');
        this.SkeletonBar('34%');
        this.SkeletonBar('65%');
        this.SkeletonBar('92%');
        this.SkeletonBar('40%');
        this.SkeletonBar('70%');
        this.SkeletonBar('14%');
        this.SkeletonBar('28%');
        this.SkeletonBar('58%');
        this.SkeletonBar('90%');
        this.SkeletonBar('34%');
      }
      .width('100%')
      .padding({ top: 30, bottom: 16 })
    }
    .layoutWeight(1)
    .width('100%')
    .scrollable(ScrollDirection.Vertical)
    .scrollBar(BarState.Off)
  }

  @Builder
  private SkeletonBar(width: string) {
    Text('')
      .width(width)
      .height(50)
      .borderRadius(25)
      .backgroundColor(SOFT)
  }
}
