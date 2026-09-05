import {
  InitialSyncResult,
  WorkspaceInfo
} from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { ConnectionErrorResult } from '../../services/ConnectionErrorPolicy';
import { ClipboardService } from '../../services/ClipboardService';
import { Encoding } from '../../services/Encoding';
import { MobileIdentitySnapshot, MobileIdentityStore } from '../../services/MobileIdentityStore';
import { RemoteDescriptorParser } from '../../services/RemoteDescriptorParser';
import { RemoteFileDownloadController } from '../../services/RemoteFileDownloadController';
import { RemoteModelController } from '../../services/RemoteModelController';
import {
  ConnectScanDecisionPolicy,
  DetectedRemoteUrlResult,
  DetectedUrlAction
} from '../../services/ConnectScanDecisionPolicy';
import { RemotePairingPolicy } from '../../services/RemotePairingPolicy';
import { RemoteSessionController } from '../../services/RemoteSessionController';
import { RemoteUiState } from '../../services/RemoteUiState';
import { QrScanService } from '../../services/QrScanService';
import { RemoteConnectionCoordinator, RemoteConnectionRequest } from '../../services/RemoteConnectionCoordinator';
import { RemotePageState } from '../state/RemotePageState';
import { AppRoute } from '../navigation/AppRouteContract';
import { RemoteLogger } from '../../services/RemoteLogger';

export enum RemoteConnectionState {
  Idle = 'idle',
  Parsing = 'parsing',
  Pairing = 'pairing',
  Connected = 'connected',
  Reconnecting = 'reconnecting',
  Failed = 'failed',
  Disconnected = 'disconnected'
}

export class RemoteConnectionController {
  private readonly pageState: RemotePageState;
  private readonly identity: MobileIdentityStore;
  private readonly pairing: RemotePairingPolicy;
  private readonly connection: RemoteConnectionCoordinator;
  private readonly sessions: RemoteSessionController;
  private readonly models: RemoteModelController;
  private readonly files: RemoteFileDownloadController;
  private readonly clipboard: ClipboardService;
  private readonly scanner: QrScanService;
  private readonly resetTimeline: (sessionId: string) => void;
  private readonly resetKnownState: () => void;
  private readonly startHeartbeat: () => void;
  private readonly stopHeartbeat: () => void;
  private readonly stopPolling: () => void;
  private readonly loadRecentWorkspaces: () => Promise<void>;
  private readonly clearCachedRemoteData: () => Promise<void>;
  private readonly replaceRoute: (route: AppRoute) => void;
  private readonly closeConnectSheet: () => void;
  private readonly openConnectSheet: () => void;
  private deviceId: string = Encoding.randomId('harmony');
  private autoReconnectAttempted: boolean = false;
  private failureCount: number = 0;
  private lockUntil: number = 0;
  private restoredControlTargetType: string = '';

  constructor(
    pageState: RemotePageState,
    identity: MobileIdentityStore,
    pairing: RemotePairingPolicy,
    connection: RemoteConnectionCoordinator,
    sessions: RemoteSessionController,
    models: RemoteModelController,
    files: RemoteFileDownloadController,
    clipboard: ClipboardService,
    scanner: QrScanService,
    resetTimeline: (sessionId: string) => void,
    resetKnownState: () => void,
    startHeartbeat: () => void,
    stopHeartbeat: () => void,
    stopPolling: () => void,
    loadRecentWorkspaces: () => Promise<void>,
    clearCachedRemoteData: () => Promise<void>,
    replaceRoute: (route: AppRoute) => void,
    closeConnectSheet: () => void,
    openConnectSheet: () => void
  ) {
    this.pageState = pageState;
    this.identity = identity;
    this.pairing = pairing;
    this.connection = connection;
    this.sessions = sessions;
    this.models = models;
    this.files = files;
    this.clipboard = clipboard;
    this.scanner = scanner;
    this.resetTimeline = resetTimeline;
    this.resetKnownState = resetKnownState;
    this.startHeartbeat = startHeartbeat;
    this.stopHeartbeat = stopHeartbeat;
    this.stopPolling = stopPolling;
    this.loadRecentWorkspaces = loadRecentWorkspaces;
    this.clearCachedRemoteData = clearCachedRemoteData;
    this.replaceRoute = replaceRoute;
    this.closeConnectSheet = closeConnectSheet;
    this.openConnectSheet = openConnectSheet;
  }

  getDeviceId(): string {
    return this.deviceId;
  }

  controlTargetPreference(): string {
    return this.restoredControlTargetType;
  }

  async rememberControlTargetType(controlTargetType: string): Promise<void> {
    const normalized = controlTargetType === 'room' || controlTargetType === 'account_device' ?
      controlTargetType : '';
    this.restoredControlTargetType = normalized;
    try {
      await this.identity.saveControlTargetType(normalized);
    } catch (err) {
      // The live transport is already authoritative. A persistence failure may
      // affect the next launch, but must not turn this connection into a failure.
      RemoteLogger.warn(`control target preference persistence failed: ${String(err)}`);
    }
  }

  async restore(context: Context): Promise<void> {
    await this.restoreWithReconnectPolicy(context, true);
  }

  async restoreWithoutReconnect(context: Context): Promise<void> {
    await this.restoreWithReconnectPolicy(context, false);
  }

  async reconnectRestoredPairing(): Promise<void> {
    if (this.shouldAutoReconnect()) {
      this.autoReconnectAttempted = true;
      this.setState(RemoteConnectionState.Reconnecting);
      this.pageState.setStatusText(RemoteI18n.t('status.restoringConnection'));
      await this.connect(true);
    }
  }

  private async restoreWithReconnectPolicy(context: Context, reconnect: boolean): Promise<void> {
    try {
      const snapshot: MobileIdentitySnapshot = await this.identity.init(context);
      this.deviceId = snapshot.installId;
      this.pageState.setUserId(snapshot.userId || snapshot.installId);
      this.pageState.setRemoteUrl(snapshot.remoteUrl);
      this.restoredControlTargetType = snapshot.controlTargetType === 'room' ||
        snapshot.controlTargetType === 'account_device' ? snapshot.controlTargetType : '';
      this.applyPairingProjection(snapshot.remoteUrl);
      this.models.setPreferredModelId(await this.identity.getLastModelId());
      this.failureCount = snapshot.userIdFailureCount;
      this.lockUntil = snapshot.userIdLockUntil > Date.now() ? snapshot.userIdLockUntil : 0;
      if (snapshot.userIdLockUntil > 0 && this.lockUntil === 0) {
        this.failureCount = 0;
        await this.identity.clearUserIdProtection();
      }
      this.pageState.setRemoteUrlInputVisible(snapshot.remoteUrl.length > 0);
      if (reconnect) {
        await this.reconnectRestoredPairing();
      }
    } catch (err) {
      this.pageState.setStatusText(ConnectionErrorPolicy.errorText(err));
      this.setState(RemoteConnectionState.Failed);
    }
  }

  async connect(autoReconnect: boolean = false, accountPassword: string = ''): Promise<void> {
    if (this.pageState.isBusy) {
      return;
    }
    const recovering = autoReconnect || this.pageState.connectionState === RemoteConnectionState.Reconnecting;
    try {
      RemoteLogger.info(`connect start auto=${autoReconnect ? '1' : '0'}`);
      this.pageState.setBusy(true);
      this.pageState.setLoadingHome(true);
      this.pageState.setConnectionFailureKind('');
      if (this.lockUntil > 0 && !ConnectionErrorPolicy.isLocked(this.lockUntil)) {
        this.failureCount = 0;
        this.lockUntil = 0;
        await this.identity.clearUserIdProtection();
      }
      if (!autoReconnect && ConnectionErrorPolicy.isLocked(this.lockUntil)) {
        throw new Error(RemoteI18n.f('errors.tooManyAttempts', `${ConnectionErrorPolicy.remainingLockSeconds(this.lockUntil)}`));
      }
      this.setState(this.pageState.connectionState === RemoteConnectionState.Reconnecting ?
        RemoteConnectionState.Reconnecting : RemoteConnectionState.Parsing);
      this.pageState.setStatusText(RemoteI18n.t('status.parsingUrl'));
      this.applyPairingProjection(this.pageState.remoteUrl);
      this.setState(RemoteConnectionState.Pairing);
      this.pageState.setStatusText(RemoteI18n.t('status.pairing'));
      const request: RemoteConnectionRequest = {
        remoteUrl: this.pageState.remoteUrl,
        userId: this.pageState.userId,
        deviceId: this.deviceId,
        accountPassword,
        autoReconnect
      };
      const initialSync: InitialSyncResult = await this.connection.connect(request);
      this.failureCount = 0;
      this.lockUntil = 0;
      this.applyWorkspace(initialSync.workspace);
      this.pageState.setAuthenticatedUserId(initialSync.authenticatedUserId);
      this.pageState.setHostCapabilities(initialSync.capabilities || []);
      this.pageState.setControlTarget('room', this.pageState.desktopId, this.pageState.desktopName);
      await this.rememberControlTargetType('room');
      this.sessions.setSessions(initialSync.sessions, initialSync.hasMoreSessions);
      this.setState(RemoteConnectionState.Connected);
      this.pageState.setConnectionFailureKind('');
      this.pageState.setStatusText(RemoteI18n.t('connection.connected'));
      this.closeConnectSheet();
      this.startHeartbeat();
      // Left running rather than awaited: this scan costs one round trip per
      // recent workspace, and everything it finds is an addition to a session
      // list that is already on screen. Its own failures are logged inside.
      void this.loadRecentWorkspaces();
      this.pageState.setLoadingHome(false);
      RemoteLogger.info(
        `connect success sessions=${initialSync.sessions.length} has_more=${initialSync.hasMoreSessions ? '1' : '0'}`
      );
    } catch (err) {
      if (!this.connection.isCurrentRequest()) {
        return;
      }
      const result: ConnectionErrorResult = await ConnectionErrorPolicy.connectionErrorResult(err, autoReconnect, {
        failureCount: this.failureCount,
        lockUntil: this.lockUntil
      }, this.identity);
      if (!this.connection.isCurrentRequest()) {
        return;
      }
      this.pageState.setStatusText(result.message);
      this.pageState.setConnectionFailureKind(result.failureKind);
      this.failureCount = result.failureCount;
      this.lockUntil = result.lockUntil;
      if (result.shouldShowRemoteUrlInput) {
        this.pageState.setRemoteUrlInputVisible(true);
      }
      const terminalFailure = result.failureKind === 'expired_room' ||
        result.failureKind === 'invalid_link' || result.failureKind === 'protected_user';
      this.setState(recovering && !terminalFailure ?
        RemoteConnectionState.Reconnecting : RemoteConnectionState.Failed);
      RemoteLogger.error(`connect failed stage=connection message=${ConnectionErrorPolicy.errorText(err)}`);
      if (!recovering) {
        this.openConnectSheet();
      }
    } finally {
      if (this.connection.isCurrentRequest()) {
        this.pageState.setLoadingHome(false);
        this.pageState.setBusy(false);
      }
    }
  }

  async reconnect(): Promise<void> {
    if (this.pageState.isBusy) {
      return;
    }
    this.setState(RemoteConnectionState.Reconnecting);
    this.pageState.setStatusText(RemoteI18n.t('status.reconnecting'));
    await this.connect(true);
  }

  async disconnect(clearPairing: boolean): Promise<void> {
    const clearingPreferredRoom = clearPairing && this.restoredControlTargetType === 'room';
    this.connection.invalidate();
    this.stopPolling();
    this.stopHeartbeat();
    this.connection.reset();
    this.sessions.clearSessions();
    this.pageState.clear();
    this.resetTimeline('');
    this.pageState.setHasMoreMessages(false);
    this.models.clearCatalog();
    this.pageState.clearActiveSession();
    this.resetKnownState();
    this.files.clear();
    this.pageState.setWorkspace(RemoteI18n.t('status.notConnected'), '', '', '', 'normal');
    this.pageState.setAuthenticatedUserId('');
    this.pageState.clearControlTarget();
    this.pageState.clearWorkspaceActions();
    this.setState(RemoteConnectionState.Disconnected);
    this.pageState.setConnectionFailureKind('');
    this.pageState.setStatusText(clearPairing ? RemoteI18n.t('status.pairingCleared') : RemoteI18n.t('status.disconnected'));
    this.replaceRoute(AppRoute.RemoteHome);
    if (clearPairing) {
      this.pageState.setRemoteUrl('');
      this.pageState.setDesktopIdentity('', '');
      this.pageState.setUserId(this.deviceId);
      this.pageState.setAccountPairing(false, '');
      this.pageState.setRemoteUrlInputVisible(false);
      await this.identity.clearPairingInput();
      if (clearingPreferredRoom) {
        await this.rememberControlTargetType('');
      }
      // Unpairing is the user saying they are done with that desktop. Leaving
      // its sessions on disk would put them back on Remote Home at the next
      // launch, with nothing behind them to open.
      await this.clearCachedRemoteData();
    } else {
      this.pageState.setRemoteUrlInputVisible(this.pageState.remoteUrl.trim().length > 0);
    }
  }

  async paste(context: Context): Promise<void> {
    try {
      this.pageState.setStatusText(RemoteI18n.t('status.readClipboard'));
      const text = (await this.clipboard.readText(context)).trim();
      if (text.length === 0) {
        this.pageState.setRemoteUrlInputVisible(true);
        this.pageState.setStatusText(RemoteI18n.t('status.clipboardEmpty'));
        return;
      }
      this.applyRemoteUrl(text);
      try {
        RemoteDescriptorParser.parse(text);
        this.pageState.setStatusText(RemoteI18n.t('status.clipboardUrlFilled'));
      } catch (_err) {
        this.pageState.setStatusText(RemoteI18n.t('status.clipboardNeedsConfirm'));
      }
    } catch (err) {
      this.pageState.setRemoteUrlInputVisible(true);
      this.pageState.setStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  async scan(context: Context): Promise<string> {
    try {
      this.pageState.setStatusText(RemoteI18n.t('status.openScanner'));
      const text = (await this.scanner.scanRemoteUrl(context)).trim();
      if (text.length === 0) {
        this.pageState.setStatusText(RemoteI18n.t('status.scanEmpty'));
        return '';
      }
      return text;
    } catch (err) {
      this.pageState.setRemoteUrlInputVisible(true);
      this.pageState.setStatusText(ConnectionErrorPolicy.errorText(err));
      return '';
    }
  }

  /**
   * Apply a scanned pair URL and say what should happen next.
   *
   * This method must not start a connect and must not close the sheet: the
   * caller owns both, so a camera callback cannot pair twice and cannot jump
   * off the progress the sheet is showing.
   */
  handleDetectedUrl(
    remoteUrl: string,
    hasCloudAccountSession: boolean = false,
    cloudUsername: string = '',
    cloudRelayUrl: string = ''
  ): DetectedRemoteUrlResult {
    try {
      this.applyRemoteUrl(remoteUrl);
      const descriptor = RemoteDescriptorParser.parse(remoteUrl);
      this.pageState.setStatusText(RemoteI18n.t('status.scannedUrl'));
      const action = ConnectScanDecisionPolicy.decide(
        descriptor.accountAuth,
        hasCloudAccountSession,
        cloudUsername,
        descriptor.accountUsername,
        RemoteUiState.desktopIdFromRemoteUrl(remoteUrl),
        cloudRelayUrl,
        descriptor.relayUrl
      );
      if (action === DetectedUrlAction.PROMPT_ACCOUNT_PASSWORD) {
        this.pageState.setStatusText(RemoteI18n.t('connect.enterAccountToPair'));
        this.openConnectSheet();
        return new DetectedRemoteUrlResult(action);
      }
      return new DetectedRemoteUrlResult(
        action,
        action === DetectedUrlAction.USE_CLOUD_DEVICE ?
          RemoteUiState.desktopIdFromRemoteUrl(remoteUrl) : ''
      );
    } catch (err) {
      this.pageState.setRemoteUrlInputVisible(true);
      this.pageState.setStatusText(ConnectionErrorPolicy.errorText(err));
      return new DetectedRemoteUrlResult(DetectedUrlAction.INVALID);
    }
  }

  ensureAvailable(): boolean {
    if (RemoteUiState.canUseRemote(this.pageState.connectionState)) {
      return true;
    }
    this.pageState.setStatusText(RemoteI18n.t('status.remoteUnavailable'));
    return false;
  }

  projectRemoteUrl(remoteUrl: string): void {
    this.applyPairingProjection(remoteUrl);
  }

  applyWorkspace(workspace: WorkspaceInfo): void {
    this.pageState.setWorkspace(workspace.name, workspace.path, workspace.assistantId || '', workspace.gitBranch,
      workspace.workspaceKind || 'normal');
  }

  private applyRemoteUrl(remoteUrl: string): void {
    this.pageState.setRemoteUrl(remoteUrl);
    this.applyPairingProjection(remoteUrl);
    this.pageState.setConnectionFailureKind('');
    this.pageState.setRemoteUrlInputVisible(true);
  }

  private applyPairingProjection(remoteUrl: string): void {
    this.pageState.setDesktopIdentity(
      remoteUrl.trim().length > 0 ? RemoteUiState.desktopNameFromRemoteUrl(remoteUrl) : this.pageState.desktopName,
      remoteUrl.trim().length > 0 ? RemoteUiState.desktopIdFromRemoteUrl(remoteUrl) : ''
    );
    const projection = this.pairing.projection(remoteUrl);
    this.pageState.setAccountPairing(projection.requiresAccountAuth, projection.accountUsername);
    const projectedUserId = this.pairing.userIdAfterProjection(this.pageState.userId, this.deviceId, projection);
    if (projectedUserId !== this.pageState.userId) {
      this.pageState.setUserId(projectedUserId);
    }
  }

  private shouldAutoReconnect(): boolean {
    return this.pairing.shouldAutoReconnect({
      autoReconnectAttempted: this.autoReconnectAttempted,
      remoteUrl: this.pageState.remoteUrl,
      userId: this.pageState.userId,
      requiresAccountAuth: this.pageState.requiresAccountAuth
    });
  }

  private setState(state: RemoteConnectionState): void {
    this.pageState.setConnectionState(state);
  }
}
