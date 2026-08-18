import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { ACCENT, CARD, GREEN, INK, LINE, MUTED, PAGE_BG, RED, SOFT } from './Theme';
import { AccountDeviceSelectionPolicy } from '../policy/AccountDeviceSelectionPolicy';

@ComponentV2
export struct ConnectAccountDevicePage {
  @Param deviceId: string = '';
  @Param controlTargetDeviceId: string = '';
  @Param connectionState: string = 'idle';
  @Event onBack: () => void = () => {};
  @Event onOpenScanner: () => void = () => {};
  @Event cloudListDevices: () => Promise<CloudAccountDevice[]> =
    async (): Promise<CloudAccountDevice[]> => [];
  @Event cloudSelectDevice: (device: CloudAccountDevice) => Promise<void> =
    async (_device: CloudAccountDevice): Promise<void> => {};
  @Local accountDevices: CloudAccountDevice[] = [];
  @Local accountDevicesBusy: boolean = false;
  @Local accountDevicesError: string = '';
  @Local switchingDeviceId: string = '';

  aboutToAppear(): void {
    this.refreshAccountDevices();
  }

  build() {
    Column() {
      Row({ space: 16 }) {
        Stack() {
          // No .width/.height here on purpose. A chevron's natural advance box
          // is about half as wide as it is tall, and SymbolGlyph draws the glyph
          // left-anchored inside a box you force wider than that — so forcing a
          // square 26vp box pushed this arrow ~7vp left of the circle's centre.
          // Left unsized, the Stack centres the natural box and the arrow sits
          // where the circle says it should.
          SymbolGlyph($r('sys.symbol.chevron_left'))
            .fontSize(23).fontColor([INK])
        }
        .width(48).height(48).backgroundColor(SOFT).borderRadius(24)
        .onClick(() => this.onBack())
        Column({ space: 4 }) {
          Text(RemoteI18n.t('connect.accountDevicesTitle'))
            .fontSize(22).fontWeight(FontWeight.Bold).fontColor(INK).width('100%')
          Text(RemoteI18n.t('connect.accountDevicesSubtitle'))
            .fontSize(13).fontColor(MUTED).width('100%')
        }
        .layoutWeight(1)
        .alignItems(HorizontalAlign.Start)
      }
      .width('100%').height(92)
      .padding({ left: 28, right: 28, top: 18 })
      .alignItems(VerticalAlign.Top)

      Scroll() {
        Column({ space: 18 }) {
          Text(RemoteI18n.t('connect.accountDevicesBody'))
            .fontSize(14).lineHeight(21).fontColor(MUTED).width('100%')
          this.AccountDeviceList()
          this.ScanPairCodeAction()
        }
        .width('100%')
        .constraintSize({ minHeight: '100%' })
        .padding({ left: 28, right: 28, top: 10, bottom: 34 })
      }
      .layoutWeight(1)
      .width('100%')
      .scrollBar(BarState.Off)
    }
    .width('100%').height('100%').backgroundColor(PAGE_BG)
  }

  @Builder
  private AccountDeviceList() {
    Column({ space: 4 }) {
      Row() {
        Text(RemoteI18n.t('connect.availableDevices'))
          .fontSize(16).fontWeight(FontWeight.Bold).fontColor(INK)
        Blank()
        Text(this.accountDevicesBusy ? RemoteI18n.t('common.loading') :
          (this.accountDevicesError.length > 0 ? RemoteI18n.t('common.retry') : RemoteI18n.t('common.refresh')))
          .fontSize(14)
          .fontColor(this.accountDevicesBusy ? MUTED :
            (this.accountDevicesError.length > 0 ? RED : ACCENT))
          .onClick(async () => { await this.refreshAccountDevices(); })
      }
      .width('100%').height(38)

      if (this.accountDevicesBusy && this.accountDevices.length === 0) {
        Column() {
          this.AccountDeviceSkeletonRow()
          this.AccountDeviceSkeletonRow()
        }
        .width('100%').height(120)
      } else if (this.desktopDevices().length === 0) {
        Row() {
          Text(this.accountDevicesError || RemoteI18n.t('remote.settings.deviceEmpty'))
            .fontSize(14).lineHeight(20).fontColor(MUTED).width('100%')
        }
        .width('100%').height(120).alignItems(VerticalAlign.Center)
      } else {
        Scroll() {
          Column() {
            ForEach(this.desktopDevices(), (device: CloudAccountDevice) => {
              this.AccountConnectDeviceRow(device)
            }, (device: CloudAccountDevice): string =>
              `${device.deviceId}:${device.online ? 'online' : 'offline'}:${device.lastSeenAt || 0}:${device.deviceName}`)
          }
          .width('100%')
        }
        .width('100%').height(120).scrollBar(BarState.Off)
      }
    }
    .width('100%').height(174)
    .padding({ left: 16, right: 16, top: 8, bottom: 8 })
    .backgroundColor(CARD).borderRadius(8).border({ width: 1, color: LINE })
  }

  @Builder
  private ScanPairCodeAction() {
    Row({ space: 12 }) {
      SymbolGlyph($r('sys.symbol.link'))
        .fontSize(20).fontColor([MUTED]).width(22).height(22).opacity(0.66)
      Text(RemoteI18n.t('connect.scanPairCodeAction'))
        .fontSize(16).fontWeight(FontWeight.Medium).fontColor(INK).layoutWeight(1)
      SymbolGlyph($r('sys.symbol.chevron_right'))
        .fontSize(13).fontColor([MUTED]).opacity(0.44)
    }
    .width('100%').height(58).padding({ left: 16, right: 16 })
    .backgroundColor(CARD).borderRadius(8).border({ width: 1, color: LINE })
    .onClick(() => this.onOpenScanner())
  }

  @Builder
  private AccountDeviceSkeletonRow() {
    Row({ space: 12 }) {
      Text('').width(26).height(22).backgroundColor(SOFT).borderRadius(5)
      Column({ space: 7 }) {
        Text('').width('58%').height(12).backgroundColor(SOFT).borderRadius(4)
        Text('').width(52).height(9).backgroundColor(SOFT).borderRadius(4)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%').height(60).padding({ left: 4, right: 4 }).alignItems(VerticalAlign.Center)
  }

  @Builder
  private AccountConnectDeviceRow(device: CloudAccountDevice) {
    Row({ space: 12 }) {
      SymbolGlyph($r('sys.symbol.desktop'))
        .fontSize(22).fontColor([MUTED]).width(26).height(24).opacity(device.online ? 0.68 : 0.38)
      Column({ space: 3 }) {
        Text(device.deviceName || device.deviceId)
          .fontSize(15).fontWeight(FontWeight.Medium).fontColor(INK)
          .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(this.accountDeviceStatus(device))
          .fontSize(13).fontColor(device.online ? GREEN : MUTED)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      if (this.canSelectAccountDevice(device) && AccountDeviceSelectionPolicy.shouldShowConnectAction(
        device.online, device.deviceId, this.deviceId, this.controlTargetDeviceId, this.connectionState
      )) {
        Text(this.switchingDeviceId === device.deviceId ?
          RemoteI18n.t('remote.settings.deviceConnecting') : RemoteI18n.t('connect.connect'))
          .fontSize(14).fontColor(INK)
          .padding({ left: 10, right: 10, top: 6, bottom: 6 })
          .backgroundColor(SOFT).borderRadius(14)
      } else if (device.online) {
        SymbolGlyph($r('sys.symbol.chevron_right'))
          .fontSize(13).fontColor([MUTED]).opacity(0.44)
      }
    }
    .width('100%').height(60).padding({ left: 4, right: 4 })
    .alignItems(VerticalAlign.Center)
    .opacity(this.canSelectAccountDevice(device) ? 1 : 0.64)
    .onClick(async () => {
      if (!this.canSelectAccountDevice(device)) return;
      this.switchingDeviceId = device.deviceId;
      this.accountDevicesError = '';
      try {
        await this.cloudSelectDevice(device);
      } catch (err) {
        this.accountDevicesError = err instanceof Error ? err.message :
          RemoteI18n.t('remote.settings.deviceSwitchFailed');
      } finally {
        this.switchingDeviceId = '';
      }
    })
  }

  private async refreshAccountDevices(): Promise<void> {
    if (this.accountDevicesBusy) return;
    this.accountDevicesBusy = true;
    this.accountDevicesError = '';
    try {
      this.accountDevices = await this.cloudListDevices();
    } catch (err) {
      this.accountDevicesError = err instanceof Error ? err.message :
        RemoteI18n.t('remote.settings.deviceLoadFailed');
    } finally {
      this.accountDevicesBusy = false;
    }
  }

  private desktopDevices(): CloudAccountDevice[] {
    return this.accountDevices.filter((device: CloudAccountDevice): boolean =>
      device.deviceId !== this.deviceId && device.deviceName !== 'HarmonyOS Phone');
  }

  private canSelectAccountDevice(device: CloudAccountDevice): boolean {
    return AccountDeviceSelectionPolicy.canSelectOnline(
      device.online, device.deviceId, this.deviceId, this.switchingDeviceId
    );
  }

  private accountDeviceStatus(device: CloudAccountDevice): string {
    if (device.deviceId === this.switchingDeviceId) {
      return RemoteI18n.t('remote.settings.deviceConnecting');
    }
    const presence = device.online ? RemoteI18n.t('remote.settings.deviceOnline') :
      RemoteI18n.t('remote.settings.deviceOffline');
    if (device.deviceId === this.controlTargetDeviceId && this.connectionState === 'connected') {
      return `${RemoteI18n.t('remote.settings.deviceControlling')} · ${presence}`;
    }
    if (device.deviceId === this.controlTargetDeviceId) {
      return `${RemoteI18n.t('connect.deviceLastUsed')} · ${presence}`;
    }
    return presence;
  }
}

