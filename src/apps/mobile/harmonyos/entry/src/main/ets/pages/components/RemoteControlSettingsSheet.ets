import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, GREEN, INK, LINE, MUTED } from './Theme';

const REMOTE_SETTINGS_BG: string = '#F4F4F7';
const REMOTE_SETTINGS_BLUE: string = '#0A84FF';

@Component
export struct RemoteControlSettingsSheet {
  @Prop desktopName: string = '';
  @Prop desktopId: string = '';
  @Prop userId: string = '';
  @Prop accountUsername: string = '';
  @Prop authenticatedUserId: string = '';
  @Prop deviceId: string = '';
  @Prop connectionState: string = 'idle';
  @Prop statusText: string = '';
  @Prop isBusy: boolean = false;
  onClose: () => void = () => {};
  onAddConnection: () => void = () => {};
  onDisconnect: () => void = () => {};
  onReconnect: () => void = () => {};
  @State showProfile: boolean = false;

  build() {
    Stack({ alignContent: Alignment.TopEnd }) {
      if (this.showProfile) {
        this.ProfilePage()
      } else {
        this.RemoteControlPage()
      }
      this.CloseButton()
    }
    .width('100%')
    .height('100%')
    .backgroundColor(REMOTE_SETTINGS_BG)
    .borderRadius({ topLeft: 34, topRight: 34 })
  }

  @Builder
  private RemoteControlPage() {
    Scroll() {
      Column({ space: 0 }) {
        Text(RemoteI18n.t('remote.settings.title'))
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
          .height(56)
          .textAlign(TextAlign.Center)
          .margin({ bottom: 30 })

        this.ProfileEntry()

        this.ConnectionSectionHeader()
        this.ConnectionCard()
      }
      .width('100%')
      .height('100%')
      .padding({ left: 18, right: 18, top: 20, bottom: 42 })
    }
    .width('100%')
    .height('100%')
    .scrollBar(BarState.Off)
  }

  @Builder
  private ProfileEntry() {
    Row({ space: 12 }) {
      Image($r('app.media.settings_profile_avatar_gw'))
        .width(34)
        .height(34)
        .objectFit(ImageFit.Contain)
      Text(RemoteI18n.t('remote.settings.profile'))
        .fontSize(18)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .layoutWeight(1)
      Image($r('app.media.settings_chevron_right'))
        .width(10)
        .height(14)
        .objectFit(ImageFit.Contain)
        .opacity(0.52)
    }
    .width('100%')
    .height(64)
    .padding({ left: 18, right: 18 })
    .backgroundColor(CARD)
    .borderRadius(28)
    .margin({ bottom: 28 })
    .onClick(() => {
      this.showProfile = true;
    })
  }

  @Builder
  private ConnectionSectionHeader() {
    Row() {
      Text(RemoteI18n.t('remote.settings.connections'))
        .fontSize(18)
        .fontWeight(FontWeight.Bold)
        .fontColor('#929298')
      Blank()
      if (this.isConnectedOrConnecting()) {
        Text(RemoteI18n.t('remote.settings.disconnectAll'))
          .fontSize(16)
          .fontColor(REMOTE_SETTINGS_BLUE)
          .onClick(() => {
            this.onDisconnect();
          })
      }
    }
    .width('100%')
    .height(42)
    .padding({ left: 18, right: 16 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private ConnectionCard() {
    Column() {
      Row({ space: 14 }) {
        Image($r('app.media.remote_ref_device'))
          .width(28)
          .height(24)
          .objectFit(ImageFit.Contain)
          .opacity(0.58)
        Column({ space: 2 }) {
          Text(RemoteI18n.t('remote.settings.desktopProduct'))
            .fontSize(14)
            .fontColor('#929298')
          Text(this.connectionTitle())
            .fontSize(18)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
          Text(this.connectionDetail())
            .fontSize(14)
            .fontColor('#929298')
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .layoutWeight(1)
        .alignItems(HorizontalAlign.Start)
        Toggle({ type: ToggleType.Switch, isOn: this.isConnectedOrConnecting() })
          .selectedColor(GREEN)
          .enabled(!this.isBusy)
          .onChange((isOn: boolean) => {
            if (isOn) {
              this.onReconnect();
            } else {
              this.onDisconnect();
            }
          })
      }
      .width('100%')
      .height(92)
      .padding({ left: 18, right: 14 })
      .alignItems(VerticalAlign.Center)

      Divider()
        .strokeWidth(1)
        .color(LINE)
        .margin({ left: 18, right: 18 })

      Row({ space: 14 }) {
        Text('+')
          .fontSize(32)
          .fontWeight(FontWeight.Lighter)
          .fontColor(REMOTE_SETTINGS_BLUE)
          .width(28)
          .textAlign(TextAlign.Center)
        Text(RemoteI18n.t('remote.settings.addConnection'))
          .fontSize(18)
          .fontWeight(FontWeight.Medium)
          .fontColor(REMOTE_SETTINGS_BLUE)
      }
      .width('100%')
      .height(62)
      .padding({ left: 18, right: 18 })
      .onClick(() => {
        this.onAddConnection();
      })
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(28)
  }

  @Builder
  private ProfilePage() {
    Scroll() {
      Column({ space: 0 }) {
        Row() {
          Button('‹')
            .width(44)
            .height(44)
            .fontSize(32)
            .fontColor(INK)
            .padding(0)
            .type(ButtonType.Circle)
            .backgroundColor(CARD)
            .onClick(() => {
              this.showProfile = false;
            })
          Text(RemoteI18n.t('remote.settings.profile'))
            .fontSize(20)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .layoutWeight(1)
            .textAlign(TextAlign.Center)
          Blank().width(44)
        }
        .width('100%')
        .height(56)
        .margin({ bottom: 30 })

        Column({ space: 10 }) {
          Image($r('app.media.settings_profile_avatar_gw'))
            .width(70)
            .height(70)
            .objectFit(ImageFit.Contain)
          Text(this.profileName())
            .fontSize(22)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .width('88%')
            .textAlign(TextAlign.Center)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
          Text(this.profileIdentifier())
            .fontSize(14)
            .fontColor(MUTED)
            .width('88%')
            .textAlign(TextAlign.Center)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .width('100%')
        .padding({ top: 24, bottom: 24 })
        .backgroundColor(CARD)
        .borderRadius(28)
        .margin({ bottom: 24 })

        Text(RemoteI18n.t('remote.settings.profileDetails'))
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
          .fontColor('#929298')
          .width('100%')
          .margin({ left: 18, bottom: 8 })

        Column() {
          this.ProfileDetailRow(RemoteI18n.t('remote.settings.userId'), this.profileIdentifier())
          Divider().strokeWidth(1).color(LINE).margin({ left: 18, right: 18 })
          this.ProfileDetailRow(RemoteI18n.t('remote.settings.deviceId'), this.deviceId || '-')
          Divider().strokeWidth(1).color(LINE).margin({ left: 18, right: 18 })
          this.ProfileDetailRow(RemoteI18n.t('remote.settings.connectedDesktop'), this.connectionTitle())
        }
        .width('100%')
        .backgroundColor(CARD)
        .borderRadius(28)
      }
      .width('100%')
      .height('100%')
      .padding({ left: 18, right: 18, top: 20, bottom: 42 })
    }
    .width('100%')
    .height('100%')
    .scrollBar(BarState.Off)
  }

  @Builder
  private ProfileDetailRow(label: string, value: string) {
    Row({ space: 16 }) {
      Text(label)
        .fontSize(16)
        .fontColor(INK)
      Blank()
      Text(value)
        .fontSize(14)
        .fontColor(MUTED)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .constraintSize({ maxWidth: 190 })
    }
    .width('100%')
    .height(58)
    .padding({ left: 18, right: 18 })
  }

  @Builder
  private CloseButton() {
    Button() {
      Image($r('app.media.settings_close_x'))
        .width(28)
        .height(28)
        .objectFit(ImageFit.Contain)
    }
    .width(50)
    .height(50)
    .padding(0)
    .type(ButtonType.Circle)
    .backgroundColor(CARD)
    .shadow({ radius: 16, color: '#10000000', offsetY: 7 })
    .margin({ top: 22, right: 18 })
    .onClick(() => {
      this.onClose();
    })
  }

  private isConnectedOrConnecting(): boolean {
    return this.connectionState === 'connected' || this.connectionState === 'reconnecting' ||
      this.connectionState === 'pairing' || this.connectionState === 'parsing';
  }

  private connectionTitle(): string {
    return this.desktopName || this.desktopId || RemoteI18n.t('remote.settings.noDesktop');
  }

  private connectionDetail(): string {
    if (this.connectionState === 'connected') {
      return RemoteI18n.t('connection.connected');
    }
    if (this.connectionState === 'reconnecting') {
      return RemoteI18n.t('connection.reconnecting');
    }
    return this.statusText || RemoteI18n.t('connection.disconnected');
  }

  private profileName(): string {
    return this.accountUsername || RemoteI18n.t('remote.settings.bitfunUser');
  }

  private profileIdentifier(): string {
    return this.authenticatedUserId || this.userId || this.deviceId || '-';
  }
}
