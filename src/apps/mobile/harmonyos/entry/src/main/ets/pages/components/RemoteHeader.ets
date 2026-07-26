import { ConnectionStatusPresenter } from '../../services/ConnectionStatusPresenter';
import { CARD, GREEN, INK, MUTED, RED } from './Theme';
import { RemoteI18n } from '../../i18n/RemoteI18n';

@ComponentV2
export struct RemoteHeader {
  @Param desktopName: string = 'MacBook Pro';
  @Param connectionState: string = 'connected';
  @Event onOpenSidebar: () => void = () => {};
  @Event onOpenActions: () => void = () => {};

  build() {
    Row() {
      this.MenuButton()
      Column({ space: 2 }) {
        Text(RemoteI18n.t('remote.title'))
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .maxLines(1)
        Row({ space: 6 }) {
          if (this.isConnecting()) {
            LoadingProgress()
              .width(14)
              .height(14)
              .color('#B9B9B9')
          } else {
            Text('')
              .width(7)
              .height(7)
              .backgroundColor(this.connectionColor())
              .borderRadius(4)
          }
          Image($r('app.media.remote_ref_device'))
            .width(19)
            .height(15)
            .objectFit(ImageFit.Contain)
          Text(this.desktopName)
            .fontSize(13)
            .fontColor(MUTED)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Center)
      this.MoreButton()
    }
    .width('100%')
    .height(68)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private MenuButton() {
    Stack({ alignContent: Alignment.Center }) {
      Image($r('app.media.remote_ref_menu'))
        .width(24)
        .height(15)
        .objectFit(ImageFit.Contain)
    }
    .width(48)
    .height(48)
    .backgroundColor(CARD)
    .borderRadius(24)
    .shadow({ radius: 18, color: '#10000000', offsetY: 8 })
    .onClick(() => {
      this.onOpenSidebar();
    })
  }

  @Builder
  private MoreButton() {
    Stack({ alignContent: Alignment.Center }) {
      Image($r('app.media.remote_ref_more'))
        .width(27)
        .height(8)
        .objectFit(ImageFit.Contain)
    }
    .width(48)
    .height(48)
    .backgroundColor(CARD)
    .borderRadius(24)
    .shadow({ radius: 18, color: '#10000000', offsetY: 8 })
    .onClick(() => {
      this.onOpenActions();
    })
  }

  private connectionColor(): string {
    const tone = ConnectionStatusPresenter.tone(this.connectionState);
    if (tone === 'ok') {
      return GREEN;
    }
    if (tone === 'error') {
      return RED;
    }
    return MUTED;
  }

  private isConnecting(): boolean {
    return this.connectionState === 'parsing' ||
      this.connectionState === 'pairing' ||
      this.connectionState === 'reconnecting';
  }
}
