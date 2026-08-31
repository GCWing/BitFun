import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, MUTED, PAGE_BG, SOFT, STATUS_DANGER } from './Theme';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { RemotePermissionMode } from '../../model/RemoteModels';
import { DefaultAccountAvatar } from './DefaultAccountAvatar';
import { BitFunAccountLoginPage } from './BitFunAccountLoginPage';
import { AccountProfilePanel } from './AccountProfilePanel';
import { SheetCloseHeader } from './SheetCloseHeader';
import { AppActionButton, AppActionStyle } from './AppActionButton';
import { SHEET_HORIZONTAL_PADDING, SHEET_TOP_RADIUS } from './SheetLayout';
import {
  RemoteControlSettingsPage,
  RemoteControlSettingsPagePolicy
} from '../policy/RemoteControlSettingsPagePolicy';

@ComponentV2
export struct RemoteControlSettingsSheet {
  @Param desktopName: string = '';
  @Param desktopId: string = '';
  @Param userId: string = '';
  @Param accountUsername: string = '';
  @Param accountUserId: string = '';
  @Param deviceId: string = '';
  @Param controlTargetType: string = 'none';
  @Param controlTargetDeviceId: string = '';
  @Param connectionState: string = 'idle';
  @Param statusText: string = '';
  @Param isBusy: boolean = false;
  @Param openAccountOnAppear: boolean = false;
  @Event onClose: () => void = () => {};
  @Event onOpenAccount: () => void = () => {};
  @Event onAddConnection: () => void = () => {};
  @Event cloudLogin: (relayUrl: string, username: string, password: string) => Promise<string> = async (_relayUrl: string, _username: string, _password: string): Promise<string> => '';
  @Event cloudLogout: () => Promise<void> = async (): Promise<void> => {};
  @Event cloudListDevices: () => Promise<CloudAccountDevice[]> = async (): Promise<CloudAccountDevice[]> => [];
  @Event cloudSelectDevice: (device: CloudAccountDevice) => Promise<void> =
    async (_device: CloudAccountDevice): Promise<void> => {};
  @Event getPermissionMode: () => Promise<RemotePermissionMode> = async (): Promise<RemotePermissionMode> => 'ask';
  @Event setPermissionMode: (mode: RemotePermissionMode) => Promise<RemotePermissionMode> =
    async (mode: RemotePermissionMode): Promise<RemotePermissionMode> => mode;
  @Event onDisconnect: () => void = () => {};
  @Event onReconnect: () => void = () => {};
  @Local showProfile: boolean = false;
  @Local showLogin: boolean = false;
  @Local permissionMode: RemotePermissionMode = 'ask';
  @Local permissionModeBusy: boolean = false;
  @Local permissionModeLoaded: boolean = false;
  @Local permissionModeError: string = '';
  @Local confirmFullAccess: boolean = false;

  aboutToAppear(): void {
    if (this.canManagePermissions()) {
      this.refreshPermissionMode();
    }
  }

  build() {
    Stack() {
      if (this.currentPage() === 'login') {
        this.LoginPage()
      } else if (this.currentPage() === 'profile') {
        this.ProfilePage()
      } else {
        this.RemoteControlPage()
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
    .borderRadius({ topLeft: SHEET_TOP_RADIUS, topRight: SHEET_TOP_RADIUS })
  }

  @Builder
  private RemoteControlPage() {
    Column({ space: 0 }) {
      SheetCloseHeader({
        title: RemoteI18n.t('remote.settings.title'),
        showDivider: true,
        onClose: this.onClose
      })

      Scroll() {
        Column({ space: 0 }) {
          this.ProfileEntry()

          this.ConnectionSectionHeader()
          this.CurrentControlCard()
          this.OtherConnectionSection()
          this.PermissionSectionHeader()
          this.PermissionModeCard()
        }
        .width('100%')
        .padding({
          left: SHEET_HORIZONTAL_PADDING,
          right: SHEET_HORIZONTAL_PADDING,
          top: 22,
          bottom: 42
        })
      }
      .width('100%')
      .layoutWeight(1)
      .scrollBar(BarState.Off)
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  private ProfileEntry() {
    Row({ space: 12 }) {
      DefaultAccountAvatar({ avatarSize: 34 })
      Text(this.isAccountAuthenticated() ? RemoteI18n.t('remote.settings.profile') :
        RemoteI18n.t('sidebar.signInBitFunAccount'))
        .fontSize(MobileDesignTypography.headlineSmall.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .layoutWeight(1)
      SymbolGlyph($r('sys.symbol.chevron_right'))
        .fontSize(14)
        .fontColor([MUTED])
        .opacity(0.52)
    }
    .width('100%')
    .height(64)
    .padding({ left: 18, right: 18 })
    .backgroundColor(CARD)
    .borderRadius(28)
    .margin({ bottom: 28 })
    .onClick(() => {
      if (this.isAccountAuthenticated()) {
        this.showProfile = true;
      } else {
        this.onOpenAccount();
      }
    })
  }

  @Builder
  private ConnectionSectionHeader() {
    Row() {
      Text(RemoteI18n.t('remote.settings.currentControl'))
        .fontSize(MobileDesignTypography.titleSmall.size)
        .lineHeight(MobileDesignTypography.titleSmall.lineHeight)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(42)
    .padding({ left: 18, right: 16 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private CurrentControlCard() {
    Column() {
      Row({ space: 14 }) {
        SymbolGlyph($r('sys.symbol.desktop'))
          .fontSize(23)
          .fontColor([MUTED])
          .width(28)
          .height(26)
          .opacity(0.58)
        Column({ space: 2 }) {
          Text(RemoteI18n.t('remote.settings.desktopProduct'))
            .fontSize(MobileDesignTypography.bodyMedium.size)
            .fontColor(MUTED)
          Text(this.connectionTitle())
            .fontSize(MobileDesignTypography.headlineSmall.size)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
          Text(this.connectionDetail())
            .fontSize(MobileDesignTypography.bodyMedium.size)
            .fontColor(MUTED)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .layoutWeight(1)
        .alignItems(HorizontalAlign.Start)
        this.ConnectionAction()
      }
      .width('100%')
      .height(92)
      .padding({ left: 18, right: 18 })
      .alignItems(VerticalAlign.Center)

      Divider()
        .strokeWidth(1)
        .color(LINE)
        .margin({ left: 18, right: 18 })

      Row({ space: 10 }) {
        SymbolGlyph($r('sys.symbol.link'))
          .fontSize(18)
          .fontColor([MUTED])
          .width(20)
          .height(20)
          .opacity(0.58)
        Text(RemoteI18n.t('remote.settings.connectionSource'))
          .fontSize(MobileDesignTypography.bodyMedium.size)
          .fontColor(MUTED)
        Blank()
        Text(this.connectionSourceLabel())
          .fontSize(MobileDesignTypography.bodySmall.size)
          .fontColor(INK)
          .padding({ left: 10, right: 10, top: 5, bottom: 5 })
          .backgroundColor(SOFT)
          .borderRadius(12)
      }
      .width('100%')
      .height(52)
      .padding({ left: 18, right: 18 })
      .alignItems(VerticalAlign.Center)
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(28)
  }

  @Builder
  private ConnectionAction() {
    if (this.isConnectedOrConnecting()) {
      Text(RemoteI18n.t('remote.settings.disconnect'))
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontColor(INK)
        .padding({ left: 10, right: 10, top: 7, bottom: 7 })
        .backgroundColor(SOFT)
        .borderRadius(14)
        .onClick(() => this.onDisconnect())
    } else if (this.hasConnectionProjection()) {
      Text(RemoteI18n.t('remote.settings.reconnect'))
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontColor(INK)
        .padding({ left: 10, right: 10, top: 7, bottom: 7 })
        .backgroundColor(SOFT)
        .borderRadius(14)
        .onClick(() => this.onReconnect())
    }
  }

  @Builder
  private OtherConnectionSection() {
    Column({ space: 10 }) {
      Text(RemoteI18n.t('remote.settings.otherConnectionMethods'))
        .fontSize(MobileDesignTypography.titleSmall.size)
        .lineHeight(MobileDesignTypography.titleSmall.lineHeight)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
        .width('100%')
        .height(42)
        .padding({ left: 18, right: 16 })
      Row({ space: 12 }) {
        SymbolGlyph($r('sys.symbol.link'))
          .fontSize(21)
          .fontColor([MUTED])
          .width(24)
          .height(24)
          .opacity(0.62)
        Column({ space: 2 }) {
          Text(RemoteI18n.t('remote.settings.qrConnect'))
            .fontSize(MobileDesignTypography.bodyLarge.size)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
          Text(RemoteI18n.t('remote.settings.qrConnectBody'))
            .fontSize(MobileDesignTypography.bodySmall.size)
            .fontColor(MUTED)
            .maxLines(2)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .layoutWeight(1)
        SymbolGlyph($r('sys.symbol.chevron_right'))
          .fontSize(14)
          .fontColor([MUTED])
          .opacity(0.52)
      }
      .width('100%')
      .height(78)
      .padding({ left: 18, right: 18, top: 10, bottom: 10 })
      .backgroundColor(CARD)
      .borderRadius(24)
      .onClick(() => this.onAddConnection())
    }
    .width('100%')
    .margin({ top: 16, bottom: 8 })
  }

  @Builder
  private PermissionSectionHeader() {
    Row() {
      Text(RemoteI18n.t('remote.permissions.title'))
        .fontSize(MobileDesignTypography.titleSmall.size)
        .lineHeight(MobileDesignTypography.titleSmall.lineHeight)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
      Blank()
      if (this.canManagePermissions()) {
        Text(RemoteI18n.t('common.refresh'))
          .fontSize(MobileDesignTypography.bodyMedium.size)
          .fontColor(this.permissionModeBusy ? MUTED : INK)
          .onClick(() => this.refreshPermissionMode())
      }
    }
    .width('100%')
    .height(54)
    .padding({ left: 18, right: 16, top: 12 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private PermissionModeCard() {
    Column() {
      Text(RemoteI18n.t('remote.permissions.scope'))
        .width('100%')
        .fontSize(MobileDesignTypography.bodySmall.size)
        .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
        .fontColor(MUTED)
        .padding({ left: 18, right: 18, top: 16, bottom: 8 })

      this.PermissionModeRow(
        'ask',
        RemoteI18n.t('remote.permissions.ask'),
        RemoteI18n.t('remote.permissions.askDescription')
      )
      Divider().strokeWidth(1).color(LINE).margin({ left: 18, right: 18 })
      this.PermissionModeRow(
        'auto',
        RemoteI18n.t('remote.permissions.auto'),
        RemoteI18n.t('remote.permissions.autoDescription')
      )
      Divider().strokeWidth(1).color(LINE).margin({ left: 18, right: 18 })
      this.PermissionModeRow(
        'full_access',
        RemoteI18n.t('remote.permissions.fullAccess'),
        RemoteI18n.t('remote.permissions.fullAccessDescription')
      )

      Text(this.permissionStatusText())
        .width('100%')
        .height(28)
        .fontSize(MobileDesignTypography.labelSmall.size)
        .fontColor(this.permissionModeError.length > 0 ? STATUS_DANGER : MUTED)
        .padding({ left: 18, right: 18, top: 4 })
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })

      if (this.confirmFullAccess) {
        this.FullAccessConfirmation()
      }
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(28)
  }

  @Builder
  private PermissionModeRow(mode: RemotePermissionMode, label: string, description: string) {
    Row({ space: 12 }) {
      Stack({ alignContent: Alignment.Center }) {
        if (this.permissionModeLoaded && this.permissionMode === mode) {
          SymbolGlyph($r('sys.symbol.checkmark_circle_fill'))
            .fontSize(19)
            .fontColor([INK])
            .width(20)
            .height(20)
        }
      }
      .width(22)
      .height(24)
      Column({ space: 3 }) {
        Text(label)
          .width('100%')
          .fontSize(MobileDesignTypography.bodyLarge.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Text(description)
          .width('100%')
          .fontSize(MobileDesignTypography.labelSmall.size)
          .lineHeight(MobileDesignTypography.labelSmall.lineHeight)
          .fontColor(MUTED)
          .maxLines(2)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .height(72)
    .padding({ left: 18, right: 18 })
    .opacity(this.canChangePermissionMode() ? 1 : 0.54)
    .onClick(() => this.selectPermissionMode(mode))
  }

  @Builder
  private FullAccessConfirmation() {
    Column({ space: 10 }) {
      Text(RemoteI18n.t('remote.permissions.fullAccessWarningTitle'))
        .width('100%')
        .fontSize(MobileDesignTypography.titleSmall.size)
        .fontWeight(FontWeight.Bold)
        .fontColor(STATUS_DANGER)
      Text(RemoteI18n.t('remote.permissions.fullAccessWarningBody'))
        .width('100%')
        .fontSize(MobileDesignTypography.bodySmall.size)
        .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
        .fontColor(INK)
      Row({ space: 10 }) {
        AppActionButton({
          label: RemoteI18n.t('remote.permissions.cancel'),
          style: AppActionStyle.Quiet,
          actionHeight: 42,
          onAction: (): void => { this.confirmFullAccess = false; }
        })
          .layoutWeight(1)
        AppActionButton({
          label: RemoteI18n.t('remote.permissions.confirmFullAccess'),
          style: AppActionStyle.Destructive,
          actionHeight: 42,
          onAction: (): void => { void this.applyPermissionMode('full_access'); }
        })
          .layoutWeight(1)
      }
      .width('100%')
    }
    .width('100%')
    .padding({ left: 16, right: 16, top: 14, bottom: 16 })
    .backgroundColor(CARD)
    .border({ width: 1, color: STATUS_DANGER })
    .borderRadius(18)
    .margin({ left: 12, right: 12, bottom: 14 })
  }

  @Builder
  private LoginPage() {
    BitFunAccountLoginPage({
      cloudLogin: this.cloudLogin,
      onClose: (): void => this.leaveAccountPage(),
      onLoginSuccess: (): void => {
        this.showLogin = false;
        this.showProfile = true;
      }
    })
  }

  @Builder
  private ProfilePage() {
    Scroll() {
      AccountProfilePanel({
        accountUsername: this.accountUsername,
        accountUserId: this.accountUserId,
        fallbackUserId: this.userId,
        deviceId: this.deviceId,
        controlTargetDeviceId: this.controlTargetDeviceId,
        connectionState: this.connectionState,
        showNavigationHeader: true,
        onBack: () => this.leaveAccountPage(),
        onLoggedOut: () => this.leaveAccountPage(),
        onReconnect: this.onReconnect,
        cloudLogout: this.cloudLogout,
        cloudListDevices: this.cloudListDevices,
        cloudSelectDevice: this.cloudSelectDevice
      })
      .padding({ left: 18, right: 18, top: 20, bottom: 42 })
    }
    .width('100%')
    .height('100%')
    .scrollBar(BarState.Off)
  }

  private isConnectedOrConnecting(): boolean {
    return this.connectionState === 'connected' || this.connectionState === 'reconnecting' ||
      this.connectionState === 'pairing' || this.connectionState === 'parsing';
  }

  private canManagePermissions(): boolean {
    return this.connectionState === 'connected';
  }

  private canChangePermissionMode(): boolean {
    return this.canManagePermissions() && !this.permissionModeBusy;
  }

  private async refreshPermissionMode(): Promise<void> {
    if (!this.canManagePermissions() || this.permissionModeBusy) {
      return;
    }
    this.permissionModeBusy = true;
    this.permissionModeError = '';
    try {
      this.permissionMode = await this.getPermissionMode();
      this.permissionModeLoaded = true;
    } catch (err) {
      this.permissionModeError = err instanceof Error ? err.message :
        RemoteI18n.t('remote.permissions.loadFailed');
    } finally {
      this.permissionModeBusy = false;
    }
  }

  private selectPermissionMode(mode: RemotePermissionMode): void {
    if (!this.canChangePermissionMode() ||
      (this.permissionModeLoaded && this.permissionMode === mode)) {
      return;
    }
    if (mode === 'full_access') {
      this.confirmFullAccess = true;
      return;
    }
    this.confirmFullAccess = false;
    this.applyPermissionMode(mode);
  }

  private async applyPermissionMode(mode: RemotePermissionMode): Promise<void> {
    if (!this.canChangePermissionMode()) {
      return;
    }
    this.permissionModeBusy = true;
    this.permissionModeError = '';
    try {
      this.permissionMode = await this.setPermissionMode(mode);
      this.permissionModeLoaded = true;
      this.confirmFullAccess = false;
    } catch (err) {
      this.permissionModeError = err instanceof Error ? err.message :
        RemoteI18n.t('remote.permissions.saveFailed');
    } finally {
      this.permissionModeBusy = false;
    }
  }

  private permissionStatusText(): string {
    if (!this.canManagePermissions()) {
      return RemoteI18n.t('remote.permissions.connectionRequired');
    }
    if (this.permissionModeBusy) {
      return RemoteI18n.t('remote.permissions.loading');
    }
    return this.permissionModeError;
  }

  private connectionTitle(): string {
    return this.desktopName || this.desktopId || RemoteI18n.t('remote.settings.noDesktop');
  }

  private connectionSourceLabel(): string {
    if (this.controlTargetType === 'account_device') {
      return RemoteI18n.t('remote.settings.accountDevice');
    }
    if (this.controlTargetType === 'room') {
      return RemoteI18n.t('remote.settings.qrPairing');
    }
    return RemoteI18n.t('remote.settings.noConnectionSource');
  }

  private hasConnectionProjection(): boolean {
    return this.controlTargetType !== 'none' || this.desktopId.trim().length > 0;
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

  private isAccountAuthenticated(): boolean {
    return this.accountUserId.trim().length > 0;
  }

  private currentPage(): RemoteControlSettingsPage {
    return RemoteControlSettingsPagePolicy.resolve(
      this.openAccountOnAppear,
      this.showLogin,
      this.showProfile,
      this.isAccountAuthenticated()
    );
  }

  private leaveAccountPage(): void {
    if (this.openAccountOnAppear) {
      this.onClose();
      return;
    }
    this.showLogin = false;
    this.showProfile = false;
  }
}
