import { abilityAccessCtrl, Context, Permissions } from '@kit.AbilityKit';
import { customScan, scanBarcode, scanCore } from '@kit.ScanKit';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { ACCENT, CARD, CONNECT_HERO_ACCENT, CONNECT_HERO_BG, CONNECT_HERO_SECONDARY,
  CONNECT_HERO_SURFACE, GREEN, INK, LINE, MUTED, PAGE_BG, PRIMARY_ACTION, PRIMARY_ACTION_TEXT, RED, SOFT,
  SUBTLE } from './Theme';
const CONNECT_SCAN_YELLOW: string = '#FFD021';
const CONNECT_OVERLAY: string = '#99000000';
const CAMERA_PERMISSION: Permissions = 'ohos.permission.CAMERA';

@Component
export struct ConnectView {
  private readonly scannerController: XComponentController = new XComponentController();
  private scannerStarted: boolean = false;
  private scanCompleted: boolean = false;
  private scanStartRetryCount: number = 0;
  @Prop remoteUrl: string = '';
  @Prop userId: string = '';
  @Prop showRemoteUrlInput: boolean = false;
  @Prop statusText: string = RemoteI18n.t('status.waitingConnection');
  @Prop connectionState: string = 'idle';
  @Prop connectionFailureKind: string = '';
  @Prop isBusy: boolean = false;
  @Prop isConnected: boolean = false;
  @Prop desktopName: string = '';
  @Prop desktopId: string = '';
  @Prop deviceId: string = '';
  @Prop accountUserId: string = '';
  @Prop controlTargetDeviceId: string = '';
  @Prop requiresAccountAuth: boolean = false;
  @Prop accountUsername: string = '';
  @Prop startWithScanner: boolean = true;
  onBack: () => void = () => {};
  onConnect: (password?: string) => void = (_password?: string) => {};
  onClearPairing: () => void = () => {};
  onRemoteUrlChange: (value: string) => void = (_value: string) => {};
  onUserIdChange: (value: string) => void = (_value: string) => {};
  onRemoteUrlDetected: (value: string) => boolean = (_value: string) => false;
  onRemoteUrlInputVisibleChange: (visible: boolean) => void = (_visible: boolean) => {};
  onPasteRemoteUrl: () => void = () => {};
  onScanRemoteUrl: () => void = () => {};
  cloudListDevices: () => Promise<CloudAccountDevice[]> = async (): Promise<CloudAccountDevice[]> => [];
  cloudSelectDevice: (device: CloudAccountDevice) => Promise<void> =
    async (_device: CloudAccountDevice): Promise<void> => {};
  @State showHelp: boolean = false;
  @State pairingStep: string = 'intro';
  @State showManualPairing: boolean = false;
  @State inlineScanError: string = '';
  @State accountPassword: string = '';
  @State cameraPermissionReady: boolean = false;
  @State requestingCameraPermission: boolean = false;
  @State accountDevices: CloudAccountDevice[] = [];
  @State accountDevicesBusy: boolean = false;
  @State accountDevicesError: string = '';
  @State switchingDeviceId: string = '';
  @State otherConnectionMethodsExpanded: boolean = false;

  aboutToAppear(): void {
    if (this.isAccountAuthenticated()) {
      this.pairingStep = 'account';
      this.refreshAccountDevices();
    } else if (this.startWithScanner && this.remoteUrl.trim().length === 0) {
      this.pairingStep = 'scan';
    }
  }

  aboutToDisappear(): void {
    this.stopInlineScan();
  }

  build() {
    Stack() {
      if (this.currentStep() === 'account') {
        this.AccountDeviceSelectionPage()
      } else if (this.currentStep() === 'scan') {
        this.ScanPairingPage()
      } else {
        this.IntroPairingPage()
      }
      if (this.showManualPairing) {
        this.ManualPairingOverlay()
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(CARD)
  }

  @Builder
  AccountDeviceSelectionPage() {
    Column() {
      Row({ space: 16 }) {
        Stack() {
          this.BackGlyph()
        }
        .width(48)
        .height(48)
        .backgroundColor(SOFT)
        .borderRadius(24)
        .onClick(() => {
          this.stopInlineScan();
          this.onBack();
        })
        Column({ space: 4 }) {
          Text(RemoteI18n.t('connect.accountDevicesTitle'))
            .fontSize(22)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .width('100%')
          Text(RemoteI18n.t('connect.accountDevicesSubtitle'))
            .fontSize(13)
            .fontColor(MUTED)
            .width('100%')
        }
        .layoutWeight(1)
        .alignItems(HorizontalAlign.Start)
      }
      .width('100%')
      .height(92)
      .padding({ left: 28, right: 28, top: 18 })
      .alignItems(VerticalAlign.Top)

      Scroll() {
        Column({ space: 18 }) {
          Text(RemoteI18n.t('connect.accountDevicesBody'))
            .fontSize(14)
            .lineHeight(21)
            .fontColor(MUTED)
            .width('100%')

          this.AccountDeviceList()
          this.OtherConnectionMethods()
        }
        .width('100%')
        .constraintSize({ minHeight: '100%' })
        .padding({ left: 28, right: 28, top: 10, bottom: 34 })
      }
      .layoutWeight(1)
      .width('100%')
      .scrollBar(BarState.Off)
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  AccountDeviceList() {
    Column({ space: 4 }) {
      Row() {
        Text(RemoteI18n.t('connect.availableDevices'))
          .fontSize(16)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
        Blank()
        Text(this.accountDevicesBusy ? RemoteI18n.t('common.loading') :
          (this.accountDevicesError.length > 0 ? RemoteI18n.t('common.retry') : RemoteI18n.t('common.refresh')))
          .fontSize(14)
          .fontColor(this.accountDevicesBusy ? MUTED :
            (this.accountDevicesError.length > 0 ? RED : ACCENT))
          .onClick(async () => {
            await this.refreshAccountDevices();
          })
      }
      .width('100%')
      .height(38)

      if (this.accountDevicesBusy && this.accountDevices.length === 0) {
        Column() {
          this.AccountDeviceSkeletonRow()
          this.AccountDeviceSkeletonRow()
        }
        .width('100%')
        .height(120)
      } else if (this.desktopDevices().length === 0) {
        Row() {
          Text(this.accountDevicesError || RemoteI18n.t('remote.settings.deviceEmpty'))
            .fontSize(14).lineHeight(20).fontColor(MUTED).width('100%')
        }
        .width('100%')
        .height(120)
        .alignItems(VerticalAlign.Center)
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
        .width('100%')
        .height(120)
        .scrollBar(BarState.Off)
      }

    }
    .width('100%')
    .height(174)
    .padding({ left: 16, right: 16, top: 8, bottom: 8 })
    .backgroundColor(CARD)
    .borderRadius(8)
    .border({ width: 1, color: LINE })
  }

  @Builder
  OtherConnectionMethods() {
    Column() {
      Row({ space: 12 }) {
        Text(RemoteI18n.t('connect.otherConnectionMethods'))
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        if (this.otherConnectionMethodsExpanded) {
          SymbolGlyph($r('sys.symbol.chevron_up'))
            .fontSize(13)
            .fontColor([MUTED])
        } else {
          SymbolGlyph($r('sys.symbol.chevron_down'))
            .fontSize(13)
            .fontColor([MUTED])
        }
      }
      .width('100%')
      .height(58)
      .padding({ left: 16, right: 16 })
      .onClick(() => {
        this.otherConnectionMethodsExpanded = !this.otherConnectionMethodsExpanded;
      })

      if (this.otherConnectionMethodsExpanded) {
        Divider()
          .color(LINE)
          .margin({ left: 16, right: 16 })

        Row({ space: 12 }) {
          SymbolGlyph($r('sys.symbol.link'))
            .fontSize(20)
            .fontColor([MUTED])
            .width(22)
            .height(22)
            .opacity(0.66)
          Text(RemoteI18n.t('connect.scanPairCodeAction'))
            .fontSize(16)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
            .layoutWeight(1)
          SymbolGlyph($r('sys.symbol.chevron_right'))
            .fontSize(13)
            .fontColor([MUTED])
            .width(16)
            .height(16)
            .opacity(0.44)
        }
        .width('100%')
        .height(58)
        .padding({ left: 16, right: 16 })
        .onClick(() => {
          this.openScannerAfterPermission();
        })
      }
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(8)
    .border({ width: 1, color: LINE })
  }

  @Builder
  AccountDeviceSkeletonRow() {
    Row({ space: 12 }) {
      Text('')
        .width(26)
        .height(22)
        .backgroundColor(SOFT)
        .borderRadius(5)
      Column({ space: 7 }) {
        Text('')
          .width('58%')
          .height(12)
          .backgroundColor(SOFT)
          .borderRadius(4)
        Text('')
          .width(52)
          .height(9)
          .backgroundColor(SOFT)
          .borderRadius(4)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .height(60)
    .padding({ left: 4, right: 4 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  AccountConnectDeviceRow(device: CloudAccountDevice) {
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
      if (device.online) {
        SymbolGlyph($r('sys.symbol.chevron_right'))
          .fontSize(13).fontColor([MUTED]).width(16).height(16).opacity(0.44)
      }
    }
    .width('100%')
    .height(60)
    .padding({ left: 4, right: 4 })
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

  @Builder
  IntroPairingPage() {
    Column() {
      Stack() {
        this.HeroWash()
        this.BackButton()
      }
      .width('100%')
      .height(282)

      Column({ space: 15 }) {
        this.DesktopGlyph()
        Text(RemoteI18n.t('connect.obtainPairCode'))
          .fontSize(24)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .width('100%')
        Text(RemoteI18n.t('connect.obtainPairCodeBody'))
          .fontSize(17)
          .lineHeight(25)
          .fontColor(MUTED)
          .textAlign(TextAlign.Center)
          .width('84%')
        Text(RemoteI18n.t('connect.obtainPairCodeSteps'))
          .fontSize(17)
          .lineHeight(25)
          .fontColor(MUTED)
          .textAlign(TextAlign.Center)
          .width('84%')
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Center)
      .margin({ top: -10 })

      this.PrimaryPairButton(RemoteI18n.t('connect.havePairCode'))
      if (this.inlineScanError.length > 0) {
        Text(this.inlineScanError)
          .fontSize(13)
          .lineHeight(18)
          .fontColor(MUTED)
          .textAlign(TextAlign.Center)
          .width('78%')
          .margin({ bottom: 10 })
        Button(RemoteI18n.t('connect.switchManualPair'))
          .width('78%')
          .height(52)
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .backgroundColor(CARD)
          .borderRadius(28)
          .border({ width: 1.5, color: LINE })
          .margin({ bottom: 10 })
          .onClick(() => {
            this.stopInlineScan();
            this.showManualPairing = true;
            this.onRemoteUrlInputVisibleChange(true);
          })
      }
      if (this.shouldShowStatusStrip()) {
        this.StatusStrip()
      }
    }
    .width('100%')
    .height('100%')
    .padding({ bottom: 34 })
    .backgroundColor(CARD)
  }

  @Builder
  ScanPairingPage() {
    Column() {
      Stack() {
        this.HeroWash()
        this.BackButton()
      }
      .width('100%')
      .height(252)

      Column({ space: 22 }) {
        this.CameraFrame()
        Text(RemoteI18n.t('connect.scanPairCode'))
          .fontSize(24)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .width('100%')
        if (this.shouldShowStatusStrip()) {
          this.StatusStrip()
        }
        if (this.inlineScanError.length > 0) {
          Text(this.inlineScanError)
            .fontSize(13)
            .lineHeight(18)
            .fontColor(MUTED)
            .textAlign(TextAlign.Center)
            .width('78%')
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Center)
      .margin({ top: -50 })

      Button(RemoteI18n.t('connect.switchManualPair'))
        .width('78%')
        .height(58)
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .backgroundColor(CARD)
        .borderRadius(35)
        .border({ width: 1.5, color: LINE })
        .onClick(() => {
          this.stopInlineScan();
          this.showManualPairing = true;
          this.onRemoteUrlInputVisibleChange(true);
        })
    }
    .width('100%')
    .height('100%')
    .padding({ bottom: 34 })
    .backgroundColor(CARD)
  }

  @Builder
  HeroWash() {
    Stack() {
      Text('')
        .width('100%')
        .height(282)
        .backgroundColor(CONNECT_HERO_BG)
        .borderRadius(36)
      Text('')
        .width(260)
        .height(142)
        .backgroundColor(CONNECT_HERO_SURFACE)
        .opacity(0.7)
        .borderRadius(72)
        .position({ x: 112, y: 26 })
      Text('')
        .width(188)
        .height(134)
        .backgroundColor(CONNECT_HERO_ACCENT)
        .opacity(0.42)
        .borderRadius(68)
        .position({ x: -42, y: 198 })
      Text('')
        .width(188)
        .height(126)
        .backgroundColor(CONNECT_HERO_SECONDARY)
        .opacity(0.54)
        .borderRadius(64)
        .position({ x: 258, y: 0 })
    }
    .width('100%')
    .height(282)
  }

  @Builder
  BackButton() {
    Stack() {
      this.BackGlyph()
    }
      .width(48)
      .height(48)
      .backgroundColor(SOFT)
      .borderRadius(24)
      .position({ x: 28, y: 18 })
      .onClick(() => {
        if (this.currentStep() === 'scan' && this.remoteUrl.trim().length === 0 && !this.showManualPairing) {
          this.stopInlineScan();
          this.pairingStep = this.isAccountAuthenticated() ? 'account' : 'intro';
          return;
        }
        this.stopInlineScan();
        this.onBack();
      })
  }

  @Builder
  BackGlyph() {
    SymbolGlyph($r('sys.symbol.chevron_left'))
      .fontSize(21)
      .fontColor([INK])
  }

  @Builder
  DesktopGlyph() {
    Stack() {
      Text('')
        .width(58)
        .height(39)
        .borderRadius(8)
        .border({ width: 5, color: INK })
        .position({ x: 5, y: 0 })
      Text('')
        .width(26)
        .height(13)
        .border({ width: 5, color: INK })
        .position({ x: 21, y: 38 })
    }
    .width(68)
    .height(55)
    .margin({ bottom: 4 })
  }

  @Builder
  CameraFrame() {
    Stack() {
      XComponent({ id: 'remote-scan-preview', type: XComponentType.SURFACE, controller: this.scannerController })
        .width(282)
        .height(282)
        .borderRadius(40)
        .onLoad(() => {
          this.startInlineScan();
        })
        .onDestroy(() => {
          this.stopInlineScan();
        })
      Text('')
        .width(282)
        .height(282)
        .backgroundColor('#18000000')
        .borderRadius(40)
      this.ScanCorner(32, 32, true, true)
      this.ScanCorner(204, 32, false, true)
      this.ScanCorner(32, 204, true, false)
      this.ScanCorner(204, 204, false, false)
    }
    .width(282)
    .height(282)
  }

  @Builder
  PrimaryPairButton(text: string) {
    Button(text)
      .width('82%')
      .height(58)
      .fontSize(21)
      .fontWeight(FontWeight.Bold)
      .fontColor(PRIMARY_ACTION_TEXT)
      .backgroundColor(PRIMARY_ACTION)
      .borderRadius(29)
      .margin({ bottom: 14 })
      .onClick(() => {
        this.openScannerAfterPermission();
      })
  }

  @Builder
  StatusStrip() {
    Column({ space: 8 }) {
      Row({ space: 8 }) {
        Text('●')
          .fontSize(12)
          .fontColor(this.statusDotColor())
        Text(this.statusDetail())
          .fontSize(13)
          .fontColor(this.isConnectError() ? INK : MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .height(30)
      .padding({ left: 12, right: 12 })
      .backgroundColor(SOFT)
      .borderRadius(15)
      .onClick(() => {
        this.handleStatusClick();
      })
      if (this.isConnectError() && this.failureHint().length > 0) {
        Text(this.failureHint())
          .fontSize(12)
          .lineHeight(18)
          .fontColor(MUTED)
          .textAlign(TextAlign.Center)
          .width('78%')
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  ManualPairingOverlay() {
    Stack() {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor(CONNECT_OVERLAY)
        .onClick(() => {
          this.stopInlineScan();
          this.showManualPairing = false;
          this.resumeInlineScan();
        })

      Column({ space: 20 }) {
        Text(this.requiresAccountAuth ? RemoteI18n.t('connect.accountPairTitle') : RemoteI18n.t('connect.manualPair'))
          .fontSize(24)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
        Text(this.requiresAccountAuth ? RemoteI18n.t('connect.accountPairIntro') : RemoteI18n.t('connect.manualPairBody'))
          .fontSize(17)
          .lineHeight(24)
          .fontColor(MUTED)
          .width('100%')
        TextInput({ placeholder: RemoteI18n.t('connect.pairCodePlaceholder'), text: this.remoteUrl })
          .height(62)
          .fontSize(20)
          .fontColor(INK)
          .backgroundColor(SOFT)
          .borderRadius(31)
          .padding({ left: 20, right: 20 })
          .defaultFocus(true)
          .onChange((value: string) => {
            this.onRemoteUrlChange(value);
          })
        if (this.requiresAccountAuth) {
          TextInput({ placeholder: RemoteI18n.t('connect.accountUsernamePlaceholder'), text: this.displayUserIdInput() })
            .height(56)
            .fontSize(18)
            .fontColor(INK)
            .backgroundColor(SOFT)
            .borderRadius(28)
            .padding({ left: 20, right: 20 })
            .onChange((value: string) => {
              this.onUserIdChange(value);
            })
          TextInput({ placeholder: RemoteI18n.t('connect.accountPasswordPlaceholder'), text: this.accountPassword })
            .height(56)
            .fontSize(18)
            .fontColor(INK)
            .backgroundColor(SOFT)
            .borderRadius(28)
            .padding({ left: 20, right: 20 })
            .type(InputType.Password)
            .onChange((value: string) => {
              this.accountPassword = value;
            })
          Text(RemoteI18n.t('connect.accountPairBody'))
            .fontSize(13)
            .lineHeight(18)
            .fontColor(MUTED)
            .width('100%')
        }
        Row({ space: 12 }) {
          Button(RemoteI18n.t('common.cancel'))
            .layoutWeight(1)
            .height(58)
            .fontSize(19)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .backgroundColor(SOFT)
            .borderRadius(29)
            .onClick(() => {
              this.stopInlineScan();
              this.showManualPairing = false;
              this.resumeInlineScan();
            })
          Button(RemoteI18n.t('connect.pair'))
            .layoutWeight(1)
            .height(58)
            .fontSize(19)
            .fontWeight(FontWeight.Bold)
            .fontColor(this.canConnect() ? PRIMARY_ACTION_TEXT : SUBTLE)
            .backgroundColor(this.canConnect() ? PRIMARY_ACTION : SOFT)
            .borderRadius(29)
            .enabled(this.canConnect())
            .onClick(() => {
              this.ensureUserId();
              this.stopInlineScan();
              this.showManualPairing = false;
              this.onConnect(this.accountPassword);
            })
        }
        .width('100%')
      }
      .width('82%')
      .padding({ left: 28, right: 28, top: 30, bottom: 28 })
      .backgroundColor(CARD)
      .borderRadius(34)
      .border({ width: 1, color: LINE })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  HelpCard() {
    Column({ space: 6 }) {
      Text(RemoteI18n.t('connect.stepsTitle'))
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
      Text(RemoteI18n.t('connect.stepsBody'))
        .fontSize(12)
        .lineHeight(18)
        .fontColor(MUTED)
        .width('100%')
    }
    .padding(14)
    .backgroundColor(SOFT)
    .borderRadius(14)
    .border({ width: 1, color: LINE })
    .width('100%')
  }

  @Builder
  ScanCard() {
    Column({ space: 12 }) {
      Row() {
        Blank()
        Stack() {
          Text('')
            .width(72)
            .height(72)
            .borderRadius(22)
            .backgroundColor(SOFT)
          this.ScanCorner(12, 12, true, true)
          this.ScanCorner(32, 12, false, true)
          this.ScanCorner(12, 32, true, false)
          this.ScanCorner(32, 32, false, false)
        }
        .width(72)
        .height(72)
        Blank()
      }
      .width('100%')
      .height(92)

      Text(RemoteI18n.t('connect.scanTitle'))
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
        .textAlign(TextAlign.Center)
      Text(RemoteI18n.t('connect.scanBody'))
        .fontSize(13)
        .lineHeight(20)
        .fontColor(MUTED)
        .width('100%')
        .textAlign(TextAlign.Center)
    }
    .padding({ left: 18, right: 18, top: 26, bottom: 24 })
    .backgroundColor(CARD)
    .borderRadius(16)
    .width('100%')
    .border({ width: 1, color: LINE })
    .onClick(() => {
      this.onScanRemoteUrl();
    })
  }

  @Builder
  ScanCorner(x: number, y: number, isLeft: boolean, isTop: boolean) {
    Stack() {
      Text('')
        .width(42)
        .height(4)
        .borderRadius(2)
        .backgroundColor(CONNECT_SCAN_YELLOW)
        .position({ x: isLeft ? 0 : 22, y: isTop ? 0 : 60 })
      Text('')
        .width(4)
        .height(42)
        .borderRadius(2)
        .backgroundColor(CONNECT_SCAN_YELLOW)
        .position({ x: isLeft ? 0 : 60, y: isTop ? 0 : 22 })
    }
      .width(64)
      .height(64)
      .position({ x, y })
  }

  @Builder
  RemoteUrlCard() {
    Column({ space: 14 }) {
      Row() {
        Text(RemoteI18n.t('connect.userId'))
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        Text(this.remoteUrl.trim().length > 0 ? RemoteI18n.t('connect.filled') : RemoteI18n.t('connect.remoteUrlShort'))
          .fontSize(13)
          .fontColor(MUTED)
          .onClick(() => {
            if (this.remoteUrl.trim().length > 0 || this.showRemoteUrlInput) {
              this.onRemoteUrlInputVisibleChange(!this.showRemoteUrlInput);
            } else {
              this.onRemoteUrlInputVisibleChange(true);
              this.onPasteRemoteUrl();
            }
          })
      }
      .width('100%')

      TextInput({ placeholder: RemoteI18n.t('connect.userPlaceholder'), text: this.displayUserIdInput() })
        .height(56)
        .fontSize(15)
        .backgroundColor(SOFT)
        .borderRadius(14)
        .padding({ left: 16, right: 16 })
        .border({ width: 1, color: LINE })
        .defaultFocus(false)
        .onChange((value: string) => {
          this.onUserIdChange(value);
        })

      if (this.showRemoteUrlInput) {
        TextInput({ placeholder: RemoteI18n.t('connect.urlPlaceholder'), text: this.remoteUrl })
          .height(50)
          .fontSize(13)
          .backgroundColor(SOFT)
          .borderRadius(14)
          .padding(12)
          .border({ width: 1, color: LINE })
          .defaultFocus(false)
          .onChange((value: string) => {
            this.onRemoteUrlChange(value);
          })
      }

      Button(this.isBusy ? RemoteI18n.t('connect.connecting') : RemoteI18n.t('connect.connect'))
        .width('100%')
        .height(50)
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(this.canConnect() ? PRIMARY_ACTION_TEXT : SUBTLE)
        .backgroundColor(this.canConnect() ? PRIMARY_ACTION : SOFT)
        .borderRadius(14)
        .enabled(this.canConnect())
        .onClick(() => {
          this.ensureUserId();
          this.onConnect();
        })
    }
    .padding({ left: 18, right: 18, top: 18, bottom: 18 })
    .backgroundColor(CARD)
    .borderRadius(16)
    .width('100%')
    .border({ width: 1, color: LINE })
  }

  @Builder
  StatusCard() {
    Column({ space: 8 }) {
      Text(RemoteI18n.t('connect.statusTitle'))
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
        .margin({ bottom: 6 })
      this.DesktopStatus()
      if (this.isConnectError() && this.failureHint().length > 0) {
        Divider().color(LINE)
        this.FailureHint()
      }
    }
    .width('100%')
    .padding(16)
    .backgroundColor(CARD)
    .borderRadius(16)
    .border({ width: 1, color: LINE })
  }

  @Builder
  FailureHint() {
    Text(this.failureHint())
      .fontSize(12)
      .lineHeight(18)
      .fontColor(INK)
      .width('100%')
      .padding(12)
      .backgroundColor(SOFT)
      .borderRadius(14)
      .border({ width: 1, color: LINE })
  }

  @Builder
  DesktopStatus() {
    List() {
      ListItem() {
        this.DesktopStatusContent()
      }
      .height(74)
      .swipeAction(this.statusSwipeAction())
    }
    .width('100%')
    .height(74)
    .scrollBar(BarState.Off)
    .divider(null)
  }

  @Builder
  DesktopStatusContent() {
    Row() {
      Text('●')
        .fontSize(12)
        .fontColor(this.statusDotColor())
      Column({ space: 6 }) {
        Text(this.statusTitle())
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Text(this.statusDetail())
          .fontSize(12)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        if (this.remoteUrl.trim().length > 0) {
          Text(this.desktopIdText())
            .fontSize(11)
            .fontColor(SUBTLE)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      .margin({ left: 12 })
      if (this.isBusy) {
        Blank()
        Text('◌')
          .fontSize(22)
          .fontColor(INK)
      }
    }
    .width('100%')
    .height(74)
    .backgroundColor(CARD)
    .onClick(() => {
      this.handleStatusClick();
    })
  }

  @Builder
  DeleteReveal() {
    Text(RemoteI18n.t('connect.clear'))
      .fontSize(13)
      .fontColor(CARD)
      .textAlign(TextAlign.Center)
      .width(84)
      .height(74)
      .backgroundColor(RED)
      .onClick(() => {
        this.onClearPairing();
      })
  }

  private statusDotColor(): ResourceColor {
    if (this.isConnected) {
      return GREEN;
    }
    if (this.isConnectError()) {
      return RED;
    }
    if (this.isBusy || this.connectionState === 'parsing' || this.connectionState === 'pairing' || this.connectionState === 'reconnecting') {
      return INK;
    }
    return SUBTLE;
  }

  private statusTitle(): string {
    if (this.remoteUrl.trim().length === 0) {
      return RemoteI18n.t('connect.noDesktop');
    }
    return this.desktopName || RemoteI18n.t('connect.targetDesktop');
  }

  private statusDetail(): string {
    if (this.remoteUrl.trim().length === 0) {
      return RemoteI18n.t('connect.noDesktopDetail');
    }
    if (this.isConnected) {
      return RemoteI18n.f2('connect.connectedTo', this.userId || this.deviceId, this.desktopName);
    }
    if (this.isBusy || this.connectionState === 'parsing' || this.connectionState === 'pairing' || this.connectionState === 'reconnecting') {
      return this.statusText || RemoteI18n.t('connect.connecting');
    }
    if (this.isConnectError()) {
      return this.statusText || RemoteI18n.t('connect.cannotConnectDesktop');
    }
    if (this.connectionState === 'disconnected') {
      return this.statusText || RemoteI18n.t('connect.desktopDisconnected');
    }
    return RemoteI18n.t('connect.waitingDesktop');
  }

  private desktopIdText(): string {
    if (this.desktopId.trim().length === 0) {
      return RemoteI18n.t('connect.desktopIdUnavailable');
    }
    return RemoteI18n.f('connect.desktopId', this.desktopId);
  }

  private displayUserIdInput(): string {
    if (this.requiresAccountAuth && this.accountUsername.length > 0 && this.userId.trim().length === 0) {
      return this.accountUsername;
    }
    if (this.remoteUrl.trim().length === 0 && (this.userId === this.deviceId || this.userId.indexOf('harmony-') === 0)) {
      return '';
    }
    return this.userId;
  }

  private statusSwipeAction(): SwipeActionOptions {
    if (this.remoteUrl.trim().length === 0 || this.isBusy) {
      return {};
    }
    return {
      end: {
        builder: () => {
          this.DeleteReveal();
        },
        actionAreaDistance: 84,
        onAction: () => {
          this.onClearPairing();
        }
      },
      edgeEffect: SwipeEdgeEffect.None
    };
  }

  private handleStatusClick(): void {
    if (this.isBusy) {
      return;
    }
    if (this.remoteUrl.trim().length > 0) {
      if (this.requiresAccountAuth && this.accountPassword.length === 0) {
        this.showManualPairing = true;
        this.onRemoteUrlInputVisibleChange(true);
        return;
      }
      this.ensureUserId();
      this.onConnect(this.accountPassword);
      return;
    }
    this.onRemoteUrlInputVisibleChange(true);
    this.stopInlineScan();
    this.showManualPairing = true;
    this.pairingStep = 'scan';
  }

  private canConnect(): boolean {
    if (this.remoteUrl.trim().length === 0 || this.isBusy) {
      return false;
    }
    if (!this.requiresAccountAuth) {
      return true;
    }
    return this.displayUserIdInput().trim().length > 0 && this.accountPassword.length > 0;
  }

  private currentStep(): string {
    if (this.pairingStep === 'account' && this.isAccountAuthenticated()) {
      return 'account';
    }
    if (this.pairingStep === 'scan' ||
      this.remoteUrl.trim().length > 0 ||
      this.isBusy ||
      this.isConnected ||
      this.connectionState === 'parsing' ||
      this.connectionState === 'pairing' ||
      this.connectionState === 'reconnecting') {
      return 'scan';
    }
    return 'intro';
  }

  private async refreshAccountDevices(): Promise<void> {
    if (!this.isAccountAuthenticated() || this.accountDevicesBusy) return;
    this.accountDevicesBusy = true;
    this.accountDevicesError = '';
    try {
      this.accountDevices = await this.cloudListDevices();
    } catch (err) {
      this.accountDevicesError = err instanceof Error ? err.message :
        RemoteI18n.t('remote.settings.deviceLoadFailed');
    } finally {
      this.accountDevicesBusy = false;
      if (!this.hasOnlineDesktopDevice()) {
        this.otherConnectionMethodsExpanded = true;
      }
    }
  }

  private desktopDevices(): CloudAccountDevice[] {
    return this.accountDevices.filter((device: CloudAccountDevice): boolean =>
      device.deviceId !== this.deviceId && device.deviceName !== 'HarmonyOS Phone');
  }

  private canSelectAccountDevice(device: CloudAccountDevice): boolean {
    return device.online && device.deviceId !== this.deviceId && this.switchingDeviceId.length === 0;
  }

  private hasOnlineDesktopDevice(): boolean {
    const devices = this.desktopDevices();
    for (let index = 0; index < devices.length; index += 1) {
      if (devices[index].online) {
        return true;
      }
    }
    return false;
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

  private isAccountAuthenticated(): boolean {
    return this.accountUserId.trim().length > 0;
  }

  private shouldShowStatusStrip(): boolean {
    return this.remoteUrl.trim().length > 0 ||
      this.isBusy ||
      this.isConnected ||
      this.isConnectError() ||
      this.connectionState === 'disconnected' ||
      this.connectionState === 'reconnecting';
  }

  private ensureUserId(): void {
    if (this.requiresAccountAuth) {
      const username = this.displayUserIdInput().trim();
      if (username.length > 0 && this.userId.trim().length === 0) {
        this.onUserIdChange(username);
      }
      return;
    }
    if (this.userId.trim().length === 0) {
      this.onUserIdChange(this.deviceId);
    }
  }

  private async startInlineScan(): Promise<void> {
    if (this.scannerStarted ||
      this.scanCompleted ||
      this.currentStep() !== 'scan' ||
      this.showManualPairing) {
      return;
    }
    if (!this.cameraPermissionReady) {
      try {
        await this.ensureCameraPermission();
        this.cameraPermissionReady = true;
      } catch (_err) {
        this.inlineScanError = RemoteI18n.t('connect.cameraPermissionDenied');
        return;
      }
    }
    try {
      this.inlineScanError = '';
      customScan.init({
        scanTypes: [scanCore.ScanType.QR_CODE],
        enableMultiMode: false,
        enableAlbum: false
      });
      const surfaceId = this.scannerController.getXComponentSurfaceId();
      if (surfaceId.length === 0) {
        if (this.scanStartRetryCount < 6) {
          this.scanStartRetryCount += 1;
          setTimeout(() => {
            this.startInlineScan();
          }, 120);
        }
        return;
      }
      this.scanStartRetryCount = 0;
      this.scannerStarted = true;
      customScan.start({
        width: 282,
        height: 282,
        surfaceId
      }, (err, results: Array<scanBarcode.ScanResult>) => {
        if (err) {
          this.inlineScanError = RemoteI18n.t('connect.cameraUnavailable');
          return;
        }
        if (this.scanCompleted || !results || results.length === 0) {
          return;
        }
        const text = (results[0].originalValue || '').trim();
        if (text.length === 0) {
          return;
        }
        this.scanCompleted = true;
        this.stopInlineScan();
        const shouldPrompt = this.onRemoteUrlDetected(text);
        if (shouldPrompt) {
          this.showManualPairing = true;
          this.onRemoteUrlInputVisibleChange(true);
          this.pairingStep = 'scan';
          return;
        }
        this.ensureUserId();
        this.onConnect();
      });
    } catch (_err) {
      this.scannerStarted = false;
      this.inlineScanError = RemoteI18n.t('connect.cameraUnavailable');
    }
  }

  private stopInlineScan(): void {
    if (!this.scannerStarted) {
      return;
    }
    this.scannerStarted = false;
    customScan.stop().then(() => {
      return customScan.release();
    }).catch((_err: Object) => {
    });
  }

  private resumeInlineScan(): void {
    if (this.remoteUrl.trim().length > 0 || this.pairingStep !== 'scan') {
      return;
    }
    this.scanCompleted = false;
    setTimeout(() => {
      if (!this.showManualPairing) {
        void this.startInlineScan();
      }
    }, 80);
  }

  private async ensureCameraPermission(): Promise<void> {
    const atManager = abilityAccessCtrl.createAtManager();
    const context = this.permissionRequestContext();
    const result = await atManager.requestPermissionsFromUser(context, [CAMERA_PERMISSION]);
    if (result.authResults.length > 0 &&
      result.authResults[0] === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
      return;
    }
    throw new Error(RemoteI18n.t('errors.permissionDenied'));
  }

  private permissionRequestContext(): Context {
    const context = this.getUIContext().getHostContext();
    if (!context) {
      throw new Error(RemoteI18n.t('errors.permissionDenied'));
    }
    return context;
  }

  private openScannerAfterPermission(): void {
    if (this.requestingCameraPermission) {
      return;
    }
    this.stopInlineScan();
    this.scanCompleted = false;
    this.scanStartRetryCount = 0;
    this.inlineScanError = '';
    this.cameraPermissionReady = false;
    this.pairingStep = 'scan';
  }

  private isConnectError(): boolean {
    if (this.connectionState === 'failed') {
      return true;
    }
    if (this.isBusy || this.isConnected) {
      return false;
    }
    const text = this.statusText.toLowerCase();
    return text.indexOf('http') >= 0 ||
      text.indexOf('error') >= 0 ||
      text.indexOf('failed') >= 0 ||
      text.indexOf(RemoteI18n.t('common.failed')) >= 0 ||
      this.statusText === RemoteI18n.t('errors.remoteUrlMissingParams') ||
      this.statusText === RemoteI18n.t('errors.remoteUrlRequired') ||
      text.indexOf('rejected') >= 0;
  }

  private failureHint(): string {
    if (this.connectionFailureKind === 'expired_room') {
      return RemoteI18n.t('connect.hintExpiredRoom');
    }
    if (this.connectionFailureKind === 'network') {
      return RemoteI18n.t('connect.hintNetwork');
    }
    if (this.connectionFailureKind === 'protected_user') {
      return RemoteI18n.t('connect.hintProtectedUser');
    }
    if (this.connectionFailureKind === 'invalid_link') {
      return RemoteI18n.t('connect.hintInvalidLink');
    }
    return '';
  }
}
