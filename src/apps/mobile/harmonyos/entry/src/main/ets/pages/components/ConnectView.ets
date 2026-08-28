import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { ConnectAccountDevicePage } from './ConnectAccountDevicePage';
import { ConnectManualPairingOverlay } from './ConnectManualPairingOverlay';
import { CONNECT_INTENT_AUTO, CONNECT_INTENT_SCAN } from '../state/AppShellState';
import {
  ConnectSheetLandingPolicy,
  ConnectSheetStatusPolicy,
  ConnectSheetStep
} from '../policy/ConnectSheetLandingPolicy';
import { DetectedUrlAction } from '../../services/ConnectScanDecisionPolicy';
import { InlineQrScanner } from './platform/InlineQrScanner';
import { CARD, CONNECT_HERO_ACCENT, CONNECT_HERO_BG, CONNECT_HERO_SECONDARY,
  CONNECT_HERO_SURFACE, CONNECT_SCAN_ACCENT, GREEN, INK, LINE, MUTED, PRIMARY_ACTION, PRIMARY_ACTION_TEXT, RED, SOFT,
  SUBTLE } from './Theme';
@ComponentV2
export struct ConnectView {
  @Param remoteUrl: string = '';
  @Param userId: string = '';
  @Param statusText: string = RemoteI18n.t('status.waitingConnection');
  @Param connectionState: string = 'idle';
  @Param connectionFailureKind: string = '';
  @Param isBusy: boolean = false;
  @Param isConnected: boolean = false;
  @Param desktopName: string = '';
  @Param deviceId: string = '';
  @Param accountUserId: string = '';
  @Param controlTargetDeviceId: string = '';
  @Param requiresAccountAuth: boolean = false;
  @Param accountUsername: string = '';
  @Param openIntent: string = CONNECT_INTENT_AUTO;
  @Event onBack: () => void = () => {};
  @Event onOpenAccount: () => void = () => {};
  @Event onConnect: (password?: string) => void = (_password?: string) => {};
  @Event onRemoteUrlChange: (value: string) => void = (_value: string) => {};
  @Event onUserIdChange: (value: string) => void = (_value: string) => {};
  @Event onRemoteUrlDetected: (value: string) => string = (_value: string) => DetectedUrlAction.INVALID;
  @Event onRemoteUrlInputVisibleChange: (visible: boolean) => void = (_visible: boolean) => {};
  @Event cloudListDevices: () => Promise<CloudAccountDevice[]> = async (): Promise<CloudAccountDevice[]> => [];
  @Event cloudSelectDevice: (device: CloudAccountDevice) => Promise<void> =
    async (_device: CloudAccountDevice): Promise<void> => {};
  @Local pairingStep: string = 'intro';
  @Local showManualPairing: boolean = false;
  @Local inlineScanError: string = '';
  @Local accountPassword: string = '';
  @Local scanRestartRevision: number = 0;

  /**
   * SCAN intent always opens the camera. Every other automatic open lands on
   * the account device list when signed in, and on the pair-code explainer
   * when not — never on the camera by default.
   */
  aboutToAppear(): void {
    this.pairingStep = ConnectSheetLandingPolicy.initialStep(
      this.openIntent,
      this.isAccountAuthenticated()
    );
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
    ConnectAccountDevicePage({
      deviceId: this.deviceId,
      controlTargetDeviceId: this.controlTargetDeviceId,
      connectionState: this.connectionState,
      cloudListDevices: this.cloudListDevices,
      cloudSelectDevice: this.cloudSelectDevice,
      onBack: () => {
        this.onBack();
      },
      onOpenScanner: () => this.openScannerAfterPermission()
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
      .height(250)

      Column({ space: 18 }) {
        this.DesktopGlyph()
        Text(RemoteI18n.t('connect.chooseConnectionTitle'))
          .fontSize(MobileDesignTypography.displayLarge.size)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .width('100%')

        Button(RemoteI18n.t('sidebar.scanToConnect'))
          .width('82%')
          .height(58)
          .fontSize(MobileDesignTypography.labelLarge.size)
          .fontWeight(FontWeight.Bold)
          .fontColor(PRIMARY_ACTION_TEXT)
          .backgroundColor(PRIMARY_ACTION)
          .borderRadius(29)
          .onClick(() => {
            this.openScannerAfterPermission();
          })

        Button(RemoteI18n.t('sidebar.signInBitFunAccount'))
          .width('82%')
          .height(58)
          .fontSize(MobileDesignTypography.labelLarge.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .backgroundColor(CARD)
          .border({ width: 1.5, color: LINE })
          .borderRadius(29)
          .onClick(() => {
            this.onOpenAccount();
          })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Center)
      .margin({ top: -24 })

      if (this.inlineScanError.length > 0) {
        Text(this.inlineScanError)
          .fontSize(MobileDesignTypography.bodySmall.size)
          .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
          .fontColor(MUTED)
          .textAlign(TextAlign.Center)
          .width('78%')
          .margin({ bottom: 10 })
        Button(RemoteI18n.t('connect.switchManualPair'))
          .width('78%')
          .height(52)
          .fontSize(MobileDesignTypography.labelLarge.size)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .backgroundColor(CARD)
          .borderRadius(28)
          .border({ width: 1.5, color: LINE })
          .margin({ bottom: 10 })
          .onClick(() => {
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
    .padding({ bottom: 28 })
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
          .fontSize(MobileDesignTypography.displayLarge.size)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .width('100%')
        if (this.shouldShowStatusStrip()) {
          this.StatusStrip()
        }
        if (this.inlineScanError.length > 0) {
          Text(this.inlineScanError)
            .fontSize(MobileDesignTypography.bodySmall.size)
            .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
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
        .fontSize(MobileDesignTypography.labelLarge.size)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .backgroundColor(CARD)
        .borderRadius(35)
        .border({ width: 1.5, color: LINE })
        .onClick(() => {
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
        if (this.openIntent !== CONNECT_INTENT_SCAN &&
          this.currentStep() === ConnectSheetStep.Scan && this.remoteUrl.trim().length === 0 && !this.showManualPairing) {
          this.pairingStep = this.isAccountAuthenticated() ? ConnectSheetStep.Account : ConnectSheetStep.Intro;
          return;
        }
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
      InlineQrScanner({
        paused: this.showManualPairing,
        restartRevision: this.scanRestartRevision,
        onDetected: (value: string) => this.handleScannedRemoteUrl(value),
        onPermissionDenied: () => {
          this.inlineScanError = RemoteI18n.t('connect.cameraPermissionDenied');
        },
        onUnavailable: () => {
          this.inlineScanError = RemoteI18n.t('connect.cameraUnavailable');
        }
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
  ScanCorner(x: number, y: number, isLeft: boolean, isTop: boolean) {
    Stack() {
      Text('')
        .width(42)
        .height(4)
        .borderRadius(2)
        .backgroundColor(CONNECT_SCAN_ACCENT)
        .position({ x: isLeft ? 0 : 22, y: isTop ? 0 : 60 })
      Text('')
        .width(4)
        .height(42)
        .borderRadius(2)
        .backgroundColor(CONNECT_SCAN_ACCENT)
        .position({ x: isLeft ? 0 : 60, y: isTop ? 0 : 22 })
    }
    .width(64)
    .height(64)
    .position({ x, y })
  }

  @Builder
  StatusStrip() {
    Column({ space: 8 }) {
      Row({ space: 8 }) {
        Text('●')
          .fontSize(MobileDesignTypography.labelSmall.size)
          .fontColor(this.statusDotColor())
        Text(this.statusDetail())
          .fontSize(MobileDesignTypography.bodySmall.size)
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
          .fontSize(MobileDesignTypography.labelSmall.size)
          .lineHeight(MobileDesignTypography.labelSmall.lineHeight)
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
    ConnectManualPairingOverlay({
      remoteUrl: this.remoteUrl,
      userIdInput: this.displayUserIdInput(),
      password: this.accountPassword,
      requiresAccountAuth: this.requiresAccountAuth,
      canSubmit: this.canConnect(),
      onRemoteUrlChange: this.onRemoteUrlChange,
      onUserIdChange: this.onUserIdChange,
      onPasswordChange: (value: string) => { this.accountPassword = value; },
      onCancel: () => this.closeManualPairing(),
      onSubmit: () => {
        this.ensureUserId();
        this.showManualPairing = false;
        this.onConnect(this.accountPassword);
      }
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

  private displayUserIdInput(): string {
    if (this.requiresAccountAuth && this.accountUsername.length > 0 && this.userId.trim().length === 0) {
      return this.accountUsername;
    }
    if (this.remoteUrl.trim().length === 0 && (this.userId === this.deviceId || this.userId.indexOf('harmony-') === 0)) {
      return '';
    }
    return this.userId;
  }

  private handleStatusClick(): void {
    if (this.isBusy) {
      return;
    }
    if (this.remoteUrl.trim().length > 0) {
      if (this.requiresAccountAuth && this.accountPassword.length === 0) {
        if (this.isAccountAuthenticated()) {
          this.pairingStep = ConnectSheetStep.Account;
          return;
        }
        this.showManualPairing = true;
        this.onRemoteUrlInputVisibleChange(true);
        return;
      }
      this.ensureUserId();
      this.onConnect(this.accountPassword);
      return;
    }
    this.onRemoteUrlInputVisibleChange(true);
    this.showManualPairing = true;
    this.pairingStep = ConnectSheetStep.Scan;
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

  private closeManualPairing(): void {
    this.showManualPairing = false;
    this.scanRestartRevision += 1;
  }

  private currentStep(): string {
    return ConnectSheetLandingPolicy.visibleStep(this.pairingStep, this.isAccountAuthenticated());
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

  private openScannerAfterPermission(): void {
    this.inlineScanError = '';
    this.scanRestartRevision += 1;
    this.pairingStep = ConnectSheetStep.Scan;
  }

  private handleScannedRemoteUrl(text: string): void {
    const action = this.onRemoteUrlDetected(text);
    if (action === DetectedUrlAction.PROMPT_ACCOUNT_PASSWORD) {
      this.showManualPairing = true;
      this.onRemoteUrlInputVisibleChange(true);
      this.pairingStep = ConnectSheetStep.Scan;
      return;
    }
    if (action === DetectedUrlAction.SHOW_CLOUD_DEVICES ||
      action === DetectedUrlAction.USE_CLOUD_DEVICE) {
      this.pairingStep = ConnectSheetStep.Account;
      return;
    }
    if (action === DetectedUrlAction.PAIR_NOW) {
      this.ensureUserId();
      this.onConnect();
      return;
    }
    this.inlineScanError = this.statusText || RemoteI18n.t('connect.hintInvalidLink');
    this.scanRestartRevision += 1;
  }

  private isConnectError(): boolean {
    return ConnectSheetStatusPolicy.isConnectError(
      this.connectionState,
      this.isBusy,
      this.isConnected,
      this.statusText,
      RemoteI18n.t('common.failed'),
      RemoteI18n.t('errors.remoteUrlMissingParams'),
      RemoteI18n.t('errors.remoteUrlRequired')
    );
  }

  private failureHint(): string {
    const key = ConnectSheetStatusPolicy.failureHintKey(this.connectionFailureKind);
    return key.length > 0 ? RemoteI18n.t(key) : '';
  }
}
