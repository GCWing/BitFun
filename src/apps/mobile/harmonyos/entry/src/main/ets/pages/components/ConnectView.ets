import { abilityAccessCtrl, Context, Permissions } from '@kit.AbilityKit';
import { customScan, scanBarcode, scanCore } from '@kit.ScanKit';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ACCENT, CARD, GREEN, INK, LINE, MUTED, RED, SOFT, SUBTLE } from './Theme';

const CONNECT_HERO: string = '#E6EDFF';
const CONNECT_HERO_BLUE: string = '#9DB4FF';
const CONNECT_HERO_LILAC: string = '#C9C5FF';
const CONNECT_HERO_MIST: string = '#F8FAFF';
const CONNECT_SCAN_YELLOW: string = '#FFD021';
const CONNECT_OVERLAY: string = '#99000000';
const CONNECT_DISABLED: string = '#D8D6D1';
const CAMERA_PERMISSION: Permissions = 'ohos.permission.CAMERA';

@Component
export struct ConnectView {
  private readonly scannerController: XComponentController = new XComponentController();
  private scannerStarted: boolean = false;
  private scanCompleted: boolean = false;
  private scanStartRetryCount: number = 0;
  @Link remoteUrl: string;
  @Link userId: string;
  @Link showRemoteUrlInput: boolean;
  @Prop statusText: string = RemoteI18n.t('status.waitingConnection');
  @Prop connectionState: string = 'idle';
  @Prop connectionFailureKind: string = '';
  @Prop isBusy: boolean = false;
  @Prop isConnected: boolean = false;
  @Prop desktopName: string = 'MacBook Pro';
  @Prop desktopId: string = '';
  @Prop deviceId: string = '';
  onBack: () => void = () => {};
  onConnect: () => void = () => {};
  onClearPairing: () => void = () => {};
  onRemoteUrlChange: (value: string) => void = (_value: string) => {};
  onPasteRemoteUrl: () => void = () => {};
  onScanRemoteUrl: () => void = () => {};
  @State showHelp: boolean = false;
  @State pairingStep: string = 'intro';
  @State showManualPairing: boolean = false;
  @State inlineScanError: string = '';
  @State cameraPermissionReady: boolean = false;
  @State requestingCameraPermission: boolean = false;

  aboutToDisappear(): void {
    this.stopInlineScan();
  }

  build() {
    Stack() {
      if (this.currentStep() === 'scan') {
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
          .border({ width: 1.5, color: CONNECT_DISABLED })
          .margin({ bottom: 10 })
          .onClick(() => {
            this.stopInlineScan();
            this.showManualPairing = true;
            this.showRemoteUrlInput = true;
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
        .border({ width: 1.5, color: CONNECT_DISABLED })
        .onClick(() => {
          this.stopInlineScan();
          this.showManualPairing = true;
          this.showRemoteUrlInput = true;
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
        .backgroundColor(CONNECT_HERO)
        .borderRadius(36)
      Text('')
        .width(260)
        .height(142)
        .backgroundColor(CONNECT_HERO_MIST)
        .opacity(0.7)
        .borderRadius(72)
        .position({ x: 112, y: 26 })
      Text('')
        .width(188)
        .height(134)
        .backgroundColor(CONNECT_HERO_BLUE)
        .opacity(0.42)
        .borderRadius(68)
        .position({ x: -42, y: 198 })
      Text('')
        .width(188)
        .height(126)
        .backgroundColor(CONNECT_HERO_LILAC)
        .opacity(0.54)
        .borderRadius(64)
        .position({ x: 258, y: 0 })
    }
    .width('100%')
    .height(282)
  }

  @Builder
  BackButton() {
    Text('‹')
      .width(48)
      .height(48)
      .fontSize(38)
      .fontColor('#0D1A33')
      .fontWeight(FontWeight.Medium)
      .textAlign(TextAlign.Center)
      .backgroundColor('#EEF4FF')
      .borderRadius(24)
      .border({ width: 1, color: '#FFFFFF' })
      .position({ x: 36, y: 38 })
      .onClick(() => {
        if (this.currentStep() === 'scan' && this.remoteUrl.trim().length === 0 && !this.showManualPairing) {
          this.stopInlineScan();
          this.pairingStep = 'intro';
          return;
        }
        this.stopInlineScan();
        this.onBack();
      })
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
      .fontColor(CARD)
      .backgroundColor(ACCENT)
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
      .backgroundColor(this.isConnectError() ? '#FFF1F1' : SOFT)
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
        })

      Column({ space: 20 }) {
        Text(RemoteI18n.t('connect.manualPair'))
          .fontSize(24)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
        Text(RemoteI18n.t('connect.manualPairBody'))
          .fontSize(17)
          .lineHeight(24)
          .fontColor(MUTED)
          .width('100%')
        TextInput({ placeholder: RemoteI18n.t('connect.pairCodePlaceholder'), text: this.remoteUrl })
          .height(62)
          .fontSize(20)
          .fontColor(INK)
          .backgroundColor('#ECEBE8')
          .borderRadius(31)
          .padding({ left: 20, right: 20 })
          .defaultFocus(true)
          .onChange((value: string) => {
            this.remoteUrl = value;
            this.onRemoteUrlChange(value);
          })
        Row({ space: 12 }) {
          Button(RemoteI18n.t('common.cancel'))
            .layoutWeight(1)
            .height(58)
            .fontSize(19)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .backgroundColor('#ECEBE8')
            .borderRadius(29)
            .onClick(() => {
              this.stopInlineScan();
              this.showManualPairing = false;
            })
          Button(RemoteI18n.t('connect.pair'))
            .layoutWeight(1)
            .height(58)
            .fontSize(19)
            .fontWeight(FontWeight.Bold)
            .fontColor(this.canConnect() ? CARD : SUBTLE)
            .backgroundColor(this.canConnect() ? ACCENT : '#ECEBE8')
            .borderRadius(29)
            .enabled(this.canConnect())
            .onClick(() => {
              this.ensureUserId();
              this.stopInlineScan();
              this.showManualPairing = false;
              this.onConnect();
            })
        }
        .width('100%')
      }
      .width('82%')
      .padding({ left: 28, right: 28, top: 30, bottom: 28 })
      .backgroundColor('#F6F6F3')
      .borderRadius(34)
      .border({ width: 1, color: '#FFFFFF' })
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
  OrDivider() {
    Row({ space: 14 }) {
      Divider().color(LINE).layoutWeight(1)
      Text(RemoteI18n.t('connect.or'))
        .fontSize(13)
        .fontColor(MUTED)
      Divider().color(LINE).layoutWeight(1)
    }
    .width('100%')
    .padding({ left: 28, right: 28, top: 6, bottom: 6 })
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
              this.showRemoteUrlInput = !this.showRemoteUrlInput;
            } else {
              this.showRemoteUrlInput = true;
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
          this.userId = value;
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
            this.remoteUrl = value;
            this.onRemoteUrlChange(value);
          })
      }

      Button(this.isBusy ? RemoteI18n.t('connect.connecting') : RemoteI18n.t('connect.connect'))
        .width('100%')
        .height(50)
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(this.canConnect() ? CARD : SUBTLE)
        .backgroundColor(this.canConnect() ? ACCENT : '#EDEBE6')
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

  private statusDotColor(): string {
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
      this.ensureUserId();
      this.onConnect();
      return;
    }
    this.showRemoteUrlInput = true;
    this.stopInlineScan();
    this.showManualPairing = true;
    this.pairingStep = 'scan';
  }

  private canConnect(): boolean {
    return this.remoteUrl.trim().length > 0 && !this.isBusy;
  }

  private currentStep(): string {
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

  private shouldShowStatusStrip(): boolean {
    return this.remoteUrl.trim().length > 0 ||
      this.isBusy ||
      this.isConnected ||
      this.isConnectError() ||
      this.connectionState === 'disconnected' ||
      this.connectionState === 'reconnecting';
  }

  private ensureUserId(): void {
    if (this.userId.trim().length === 0) {
      this.userId = this.deviceId;
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
        this.remoteUrl = text;
        this.onRemoteUrlChange(text);
        this.ensureUserId();
        this.stopInlineScan();
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
