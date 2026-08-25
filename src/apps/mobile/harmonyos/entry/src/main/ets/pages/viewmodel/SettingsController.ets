import { RemoteI18n } from '../../i18n/RemoteI18n';
import { stampRemoteSessionsDevice } from '../../services/RemoteSessionIdentity';
import {
  CloudAccountBinding,
  CloudAccountClient,
  CloudAccountDevice,
  CloudAccountRequestError,
  CloudAccountSession
} from '../../services/CloudAccountClient';
import { CloudAccountSessionStore, PersistedCloudAccountSession } from '../../services/CloudAccountSessionStore';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { Encoding } from '../../services/Encoding';
import {
  GeneralChatConfigSnapshot,
  GeneralChatConfigStore,
  GeneralChatConfigUpdate,
  GeneralChatConfigValidator,
  GeneralChatModelSelectionPolicy
} from '../../services/general-chat/GeneralChatConfigStore';
import { GeneralChatCloudConfigPolicy } from '../../services/general-chat/GeneralChatCloudConfigPolicy';
import { GeneralChatServiceStatus } from '../../services/general-chat/GeneralChatServiceState';
import { RemoteHeartbeatController } from '../../services/RemoteHeartbeatController';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteSessionManager } from '../../services/RemoteSessionManager';
import { WatchProvisionOutcome } from '../../services/WatchProvisionController';
import { RemotePermissionMode } from '../../model/RemoteModels';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemotePageState } from '../state/RemotePageState';

export interface SettingsControllerHooks {
  readonly probeConfiguration: (apiUrl: string, apiKey: string, modelName: string) => Promise<void>;
}

export interface CloudAccountSettingsHooks {
  readonly deviceId: () => string;
  readonly remoteAvailable: () => boolean;
  readonly invalidatePreview: () => void;
  readonly invalidateRemoteActivity: () => void;
  readonly invalidateRemoteConnection: () => void;
  readonly stopPolling: () => void;
  readonly stopHeartbeat: () => void;
  readonly startHeartbeat: () => void;
  readonly resetTimeline: () => void;
  readonly resetKnownRemoteState: () => void;
  /** Cached transcripts and session lists belong to the account that was signed in. */
  readonly clearCachedRemoteData: () => Promise<void>;
  readonly closeSettings: () => void;
  readonly closeConnectSheet: () => void;
  readonly navigateRemoteHome: () => void;
  readonly loadRecentWorkspaces: () => Promise<void>;
  readonly onAccountDevices: (devices: CloudAccountDevice[]) => void;
  /** Snapshot the outgoing live device before teardown clears its lists. */
  readonly onBeforeControlTargetChange: () => void;
  readonly onControlTargetChanged: () => void;
}

export interface CloudAccountSettingsDependencies {
  readonly client: CloudAccountClient;
  readonly sessionStore: CloudAccountSessionStore;
  readonly sessionManager: RemoteSessionManager;
  readonly remoteState: RemotePageState;
  readonly hooks: CloudAccountSettingsHooks;
}

/** How often presence is refetched while device rows are on screen. */
const PRESENCE_POLL_INTERVAL_MS: number = 20000;

/** Owns general-chat model service settings and their presentation projection. */
export class SettingsController {
  private readonly store: GeneralChatConfigStore;
  private readonly state: GeneralChatPageState;
  private readonly hooks: SettingsControllerHooks;
  private readonly cloud?: CloudAccountSettingsDependencies;
  private cloudSession?: CloudAccountSession;
  private cloudRelayUrl: string = '';
  private accountDevices: CloudAccountDevice[] = [];
  private lastAccountDevicesAt: number = 0;
  private presenceTicker?: RemoteHeartbeatController;
  private restoringControlTarget: boolean = false;

  constructor(
    store: GeneralChatConfigStore,
    state: GeneralChatPageState,
    hooks: SettingsControllerHooks,
    cloud?: CloudAccountSettingsDependencies
  ) {
    this.store = store;
    this.state = state;
    this.hooks = hooks;
    this.cloud = cloud;
  }

  async save(
    apiUrl: string,
    apiKey: string,
    modelName: string,
    clearApiKey: boolean
  ): Promise<string> {
    const update = this.update(apiUrl, apiKey, modelName, clearApiKey);
    try {
      const validationError = await this.validate(update);
      if (validationError.length > 0) {
        return validationError;
      }
      if (!update.clearApiKey) {
        const probeError = await this.probe(update);
        if (probeError.length > 0) {
          return probeError;
        }
      }
      const catalogBeforeSave = await this.store.modelCatalog();
      const snapshot = await this.store.save(update);
      if (GeneralChatModelSelectionPolicy.shouldActivateSavedLocalModel(catalogBeforeSave)) {
        await this.store.selectLocalModel();
      }
      this.apply(snapshot);
      await this.refreshModelCatalog();
      return '';
    } catch (err) {
      return ConnectionErrorPolicy.errorText(err);
    }
  }

  async test(
    apiUrl: string,
    apiKey: string,
    modelName: string,
    clearApiKey: boolean
  ): Promise<string> {
    const update = this.update(apiUrl, apiKey, modelName, clearApiKey);
    try {
      const validationError = await this.validate(update);
      if (validationError.length > 0) {
        return validationError;
      }
      if (update.clearApiKey) {
        return RemoteI18n.t('settings.modelService.testNeedsKey');
      }
      return await this.probe(update);
    } catch (err) {
      return ConnectionErrorPolicy.errorText(err);
    }
  }

  apply(snapshot: GeneralChatConfigSnapshot): void {
    this.state.setConfiguration(
      snapshot.apiUrl,
      snapshot.modelName,
      snapshot.hasApiKey,
      GeneralChatServiceStatus.fromConfiguration(snapshot.apiUrl, snapshot.modelName, snapshot.hasApiKey)
    );
  }

  async refreshModelCatalog(): Promise<void> {
    const catalog = await this.store.modelCatalog();
    const selectedModelId = catalog.session_model_id || catalog.default_models.primary || '';
    this.state.setModelCatalog(catalog, selectedModelId);
    const active = await this.store.activeSnapshot();
    this.state.setServiceState(
      GeneralChatServiceStatus.fromConfiguration(active.apiUrl, active.modelName, active.hasApiKey)
    );
  }

  async selectModel(modelId: string): Promise<boolean> {
    if (!await this.store.selectModel(modelId)) {
      return false;
    }
    await this.refreshModelCatalog();
    return true;
  }

  async initializeCloudAccount(context: Context): Promise<void> {
    const cloud = this.requireCloud();
    await cloud.sessionStore.init(context);
    await this.restoreCloudAccountSession();
  }

  hasCloudAccountSession(): boolean {
    return this.cloudSession !== undefined;
  }

  /**
   * Whether this phone might be able to add a device to the account itself.
   *
   * Deliberately "might": the session here is either a real password login or
   * one delegated by a room pairing, and only the first carries a token the
   * relay will mint from. The two are indistinguishable locally, so the answer
   * is optimistic and `provisionWatchCredential` reports the refusal.
   */
  canMintWatchCredential(): boolean {
    return this.cloudSession !== undefined && this.cloudRelayUrl.length > 0;
  }

  /**
   * Mints a watch's own account credential straight from this phone.
   *
   * Returns `undefined` when this phone is not entitled to mint — no session,
   * or a delegated token the relay refuses — which is the caller's cue to fall
   * back to asking the paired desktop. Any other failure throws, because a
   * network blip is not the same answer as "you may not".
   *
   * The account master key never leaves for the relay: it is read from the
   * session this phone already holds and handed back for sealing to the
   * watch's ephemeral key.
   */
  async provisionWatchCredential(
    deviceId: string,
    deviceName: string,
    requestId: string
  ): Promise<WatchProvisionOutcome | undefined> {
    const session = this.cloudSession;
    if (!session || this.cloudRelayUrl.length === 0) {
      return undefined;
    }
    const cloud = this.requireCloud();
    try {
      const provisioned = await cloud.client.provisionDevice(
        this.cloudRelayUrl, session, deviceId, deviceName, requestId);
      RemoteLogger.info(`watch credential minted from the phone account device=${provisioned.deviceId}`);
      return {
        ok: true,
        relayUrl: this.cloudRelayUrl,
        token: provisioned.token,
        userId: provisioned.userId,
        masterKeyBase64: Encoding.bytesToBase64(session.masterKey),
        deviceId: provisioned.deviceId,
        failure: '',
        desktopReported: false
      };
    } catch (err) {
      if (err instanceof CloudAccountRequestError && (err.statusCode === 401 || err.statusCode === 403)) {
        RemoteLogger.info('phone account may not mint a device credential; deferring to the desktop');
        return undefined;
      }
      throw err instanceof Error ? err : new Error('Watch credential provisioning failed.');
    }
  }

  async persistDelegatedAccountSession(): Promise<void> {
    if (this.cloudSession) {
      return;
    }
    const cloud = this.requireCloud();
    const delegated = cloud.sessionManager.delegatedAccountSession();
    if (!delegated) {
      return;
    }
    this.applyCloudAccountSession(delegated.session, delegated.relayUrl, delegated.session.userId);
    await cloud.sessionStore.save({
      relayUrl: delegated.relayUrl,
      username: delegated.session.userId,
      token: delegated.session.token,
      userId: delegated.session.userId,
      masterKey: Encoding.bytesToBase64(delegated.session.masterKey)
    });
    RemoteLogger.info('delegated account session persisted after room pairing');
  }

  async loginCloudAccount(relayUrl: string, username: string, password: string): Promise<string> {
    const cloud = this.requireCloud();
    RemoteLogger.info('cloud account UI login requested');
    const session = await cloud.client.login(relayUrl, username, password, cloud.hooks.deviceId());
    this.applyCloudAccountSession(session, relayUrl, username);
    await cloud.sessionStore.save({
      relayUrl: relayUrl.trim(), username: username.trim(), token: session.token, userId: session.userId,
      masterKey: Encoding.bytesToBase64(session.masterKey)
    });
    await this.loadGeneralChatAccountModels(session, relayUrl);
    RemoteLogger.info('cloud account credentials persisted, refreshing account devices');
    RemoteLogger.info(`cloud account login success user=${session.userId}`);
    return session.userId;
  }

  applyCloudAccountSession(session: CloudAccountSession, relayUrl: string, username: string): void {
    const remoteState = this.requireCloud().remoteState;
    this.cloudSession = session;
    this.cloudRelayUrl = relayUrl.trim();
    remoteState.setAccountUserId(session.userId);
    remoteState.setAccountUsername(username.trim());
  }

  async logoutCloudAccount(): Promise<void> {
    const cloud = this.requireCloud();
    cloud.hooks.invalidatePreview();
    if (cloud.remoteState.controlTargetType === 'account_device') {
      this.resetAccountDeviceConnection(true);
    }
    this.clearCloudAccountProjection(cloud);
    this.store.replaceAccountModels([]);
    await this.refreshModelCatalog();
    await cloud.sessionStore.clear();
    await cloud.hooks.clearCachedRemoteData();
    cloud.remoteState.clearControlTarget();
    RemoteLogger.info('cloud account logout success');
  }

  activeCloudAccount(): CloudAccountBinding | undefined {
    const cloud = this.cloud;
    const session = this.cloudSession;
    if (!cloud || !session || this.cloudRelayUrl.length === 0) {
      return undefined;
    }
    const binding: CloudAccountBinding = {
      client: cloud.client,
      relayUrl: this.cloudRelayUrl,
      session
    };
    return binding;
  }

  /**
   * Polls presence for as long as device rows are on screen.
   *
   * Events alone do not cover the screen where this matters most: the wide
   * layout keeps the sidebar mounted in its master pane, so the drawer open
   * that carries a refresh never fires there and a desktop coming online is
   * never noticed. The relay cannot push presence, so the rows have to ask.
   * The ticker reuses the heartbeat's drop-overlapping-ticks semantics, and
   * [maxAgeMs] still gates the request, so a tick that lands right after an
   * event-driven refresh costs nothing.
   */
  startPresencePolling(maxAgeMs: number): void {
    if (!this.presenceTicker) {
      this.presenceTicker = new RemoteHeartbeatController(async (): Promise<void> => {
        await this.refreshAccountDevicesIfStale(maxAgeMs);
      }, PRESENCE_POLL_INTERVAL_MS);
    }
    this.presenceTicker.start();
  }

  /** Stops the presence poll. Nothing is on screen to go stale. */
  stopPresencePolling(): void {
    this.presenceTicker?.stop();
  }

  /**
   * Refetches account device presence when the last answer is older than
   * [maxAgeMs].
   *
   * The relay owns presence in a live socket registry, so its answer is only
   * as current as the last time we asked — and the only ask used to be at
   * cold start. A desktop that came online afterwards stayed grey in the
   * sidebar until the app was killed and relaunched.
   *
   * Failure is swallowed on purpose: this runs off returning to the
   * foreground, where the user asked for nothing and a thrown error would
   * surface as an alert about a request they never made.
   */
  async refreshAccountDevicesIfStale(maxAgeMs: number): Promise<void> {
    if (!this.hasCloudAccountSession()) {
      return;
    }
    if (Date.now() - this.lastAccountDevicesAt < maxAgeMs) {
      return;
    }
    try {
      await this.listCloudAccountDevices();
    } catch (err) {
      RemoteLogger.warn(`account device presence refresh failed: ${String(err)}`);
    }
  }

  /**
   * Re-publishes the cached device list so presence can be re-derived.
   *
   * Presence is not purely the relay's to decide: a device we hold a live link
   * to is online whatever the socket registry last projected. That override is
   * evaluated when the list is published, so a link going live has to republish
   * or the desktop we just connected to keeps whatever the last fetch claimed —
   * on a cold start that fetch happens before the reconnect lands, and the
   * answer it brought back is "offline".
   *
   * No request goes out: this is the same list, read again.
   */
  resyncAccountDevicePresence(): void {
    if (this.accountDevices.length === 0) {
      return;
    }
    this.requireCloud().hooks.onAccountDevices(this.accountDevices);
  }

  async listCloudAccountDevices(): Promise<CloudAccountDevice[]> {
    const cloud = this.requireCloud();
    const session = this.cloudSession;
    if (!session || this.cloudRelayUrl.length === 0) {
      return [];
    }
    try {
      const devices = await cloud.client.listDevices(this.cloudRelayUrl, session, cloud.hooks.deviceId());
      this.accountDevices = devices;
      this.lastAccountDevicesAt = Date.now();
      cloud.hooks.onAccountDevices(devices);
      return devices;
    } catch (err) {
      if (err instanceof CloudAccountRequestError && err.statusCode === 401) {
        await this.expireCloudAccountSession();
        throw new Error(RemoteI18n.t('remote.settings.accountExpired'));
      }
      if (err instanceof CloudAccountRequestError &&
        (err.statusCode === 404 || err.statusCode === 503 || err.statusCode === 504)) {
        throw new Error(RemoteI18n.t('remote.settings.deviceUnavailable'));
      }
      throw new Error(err instanceof Error ? err.message : RemoteI18n.t('remote.settings.deviceLoadFailed'));
    }
  }

  knownAccountDevice(deviceId: string): CloudAccountDevice | undefined {
    const target = deviceId.trim();
    if (target.length === 0) {
      return undefined;
    }
    return this.accountDevices.find((item: CloudAccountDevice): boolean => item.deviceId === target);
  }

  async getRemotePermissionMode(): Promise<RemotePermissionMode> {
    const cloud = this.requireCloud();
    if (!cloud.hooks.remoteAvailable()) {
      throw new Error(RemoteI18n.t('remote.permissions.connectionRequired'));
    }
    return cloud.sessionManager.getPermissionMode();
  }

  async setRemotePermissionMode(mode: RemotePermissionMode): Promise<RemotePermissionMode> {
    const cloud = this.requireCloud();
    if (!cloud.hooks.remoteAvailable()) {
      throw new Error(RemoteI18n.t('remote.permissions.connectionRequired'));
    }
    return cloud.sessionManager.setPermissionMode(mode);
  }

  /**
   * Puts the account back on the desktop it was last driving.
   *
   * `navigateHome` is off for the cold-start restore: the launch route is the
   * user's business, and reconnecting in the background should not move them
   * off the screen the app opened on. A manual reconnect is already on the
   * Remote surface, so there it lands where the user expects.
   */
  async restoreCloudTarget(
    targetDeviceId: string,
    targetDeviceName: string,
    navigateHome: boolean = true
  ): Promise<void> {
    const targetId = targetDeviceId.trim();
    if (targetId.length === 0) {
      return;
    }
    const remoteState = this.requireCloud().remoteState;
    try {
      const devices = await this.listCloudAccountDevices();
      const target = devices.find((device: CloudAccountDevice): boolean => device.deviceId === targetId);
      if (!target || !target.online) {
        const targetName = target?.deviceName || targetDeviceName || targetId;
        remoteState.setControlTarget('account_device', targetId, targetName);
        remoteState.setDesktopIdentity(targetName, targetId);
        remoteState.setConnectionState('failed');
        remoteState.setStatusText(RemoteI18n.t('remote.settings.deviceUnavailable'));
        return;
      }
      await this.selectCloudAccountDevice({
        deviceId: target.deviceId,
        deviceName: target.deviceName || targetDeviceName || target.deviceId,
        online: target.online,
        lastSeenAt: target.lastSeenAt
      }, navigateHome);
    } catch (err) {
      RemoteLogger.warn(`cloud target restore failed: ${err instanceof Error ? err.message : 'unknown error'}`);
      if (!this.cloudSession) {
        // The account itself went away — an expired token clears it — so there
        // is no desktop left to aim at, and the surfaces already say as much.
        return;
      }
      // A failed connect attempt drops the target so a half-open link cannot be
      // mistaken for a live one, but a restore that failed is exactly the case
      // where the target still matters: without it the reconnect button has
      // nothing to retry and falls through to the paired-desktop path.
      remoteState.setControlTarget('account_device', targetId, targetDeviceName.trim() || targetId);
      // The caller announces `reconnecting` before this runs, and every surface
      // hides its reconnect entry point while a connect is in flight. Failing
      // silently would leave that announcement standing forever, with no way
      // left to retry it. Only report when nothing downstream already did.
      if (remoteState.connectionState !== 'failed') {
        remoteState.setConnectionState('failed');
        remoteState.setStatusText(ConnectionErrorPolicy.errorText(err));
      }
    }
  }

  async handleRemoteConnectionError(err: Object): Promise<boolean> {
    const remoteState = this.requireCloud().remoteState;
    if (remoteState.controlTargetType !== 'account_device' ||
      !(err instanceof CloudAccountRequestError) || err.statusCode !== 401) {
      return false;
    }
    await this.expireCloudAccountSession();
    remoteState.setStatusText(RemoteI18n.t('remote.settings.accountExpired'));
    return true;
  }

  async selectCloudAccountDevice(device: CloudAccountDevice, navigateHome: boolean = true): Promise<void> {
    const cloud = this.requireCloud();
    const session = this.cloudSession;
    if (!device.online) {
      throw new Error(RemoteI18n.t('remote.settings.deviceOffline'));
    }
    if (!session || this.cloudRelayUrl.length === 0) {
      throw new Error(RemoteI18n.t('remote.settings.accountNotSignedIn'));
    }
    const deviceId = device.deviceId.trim();
    if (deviceId.length === 0 || deviceId === cloud.hooks.deviceId()) {
      return;
    }
    if (deviceId === cloud.remoteState.controlTargetDeviceId && cloud.remoteState.connectionState === 'connected') {
      cloud.hooks.closeConnectSheet();
      if (navigateHome) {
        cloud.hooks.navigateRemoteHome();
      }
      return;
    }
    // Taken before the teardown, which is the last moment the outgoing device
    // is still readable off the page state.
    const previousTarget = this.restorableControlTarget();
    this.prepareAccountDeviceConnection();
    try {
      const initialSync = await cloud.sessionManager.connectAccountDevice(
        cloud.client,
        this.cloudRelayUrl,
        session,
        deviceId
      );
      cloud.remoteState.setControlTarget('account_device', deviceId, device.deviceName);
      cloud.remoteState.setDesktopIdentity(device.deviceName, deviceId);
      cloud.remoteState.setWorkspace(
        initialSync.workspace.name,
        initialSync.workspace.path,
        initialSync.workspace.assistantId || '',
        initialSync.workspace.gitBranch,
        initialSync.workspace.workspaceKind || 'normal'
      );
      cloud.remoteState.setSessions(
        stampRemoteSessionsDevice(initialSync.sessions, deviceId),
        initialSync.hasMoreSessions
      );
      cloud.hooks.onControlTargetChanged();
      cloud.remoteState.setAuthenticatedUserId(initialSync.authenticatedUserId);
      cloud.remoteState.setConnectionState('connected');
      this.resyncAccountDevicePresence();
      cloud.remoteState.setStatusText(RemoteI18n.t('connection.connected'));
      // Dismissing the picker and going home are the same event: the switch the
      // user asked for from a sheet is done. A switch nobody asked for — the
      // one behind opening a session on another desktop, or the restore after a
      // failed one — must not take their place in the UI away from them.
      if (navigateHome) {
        cloud.hooks.closeSettings();
        cloud.hooks.closeConnectSheet();
        cloud.hooks.navigateRemoteHome();
      }
      await cloud.sessionStore.save({
        relayUrl: this.cloudRelayUrl,
        username: cloud.remoteState.accountUsername,
        token: session.token,
        userId: session.userId,
        masterKey: Encoding.bytesToBase64(session.masterKey),
        targetDeviceId: deviceId,
        targetDeviceName: device.deviceName
      });
      cloud.hooks.startHeartbeat();
      // Same reasoning as the paired-desktop path in RemoteConnectionController:
      // the cross-workspace scan runs behind a list that is already usable.
      void cloud.hooks.loadRecentWorkspaces();
    } catch (err) {
      if (err instanceof CloudAccountRequestError && err.statusCode === 401) {
        await this.expireCloudAccountSession();
      }
      cloud.remoteState.clearControlTarget();
      cloud.remoteState.setConnectionState('failed');
      const message = ConnectionErrorPolicy.errorText(err);
      cloud.remoteState.setStatusText(message);
      cloud.sessionManager.reset();
      await this.restoreControlTarget(previousTarget);
      throw new Error(message);
    } finally {
      cloud.remoteState.setLoadingHome(false);
      cloud.remoteState.setBusy(false);
    }
  }

  /**
   * The device to fall back to if the switch about to start fails, or nothing.
   *
   * Only an account device qualifies. A QR-paired room is re-established by
   * `RemoteConnectionController`, not from here, so offering to restore it
   * would be a promise this method cannot keep. `online: true` is not a guess:
   * it is what the live connection just proved, and it is what
   * `selectCloudAccountDevice` refuses to dial without.
   */
  private restorableControlTarget(): CloudAccountDevice | undefined {
    const state = this.requireCloud().remoteState;
    if (state.controlTargetType !== 'account_device' || state.connectionState !== 'connected') {
      return undefined;
    }
    const deviceId = state.controlTargetDeviceId.trim();
    if (deviceId.length === 0) {
      return undefined;
    }
    return {
      deviceId,
      deviceName: state.controlTargetDeviceName || deviceId,
      online: true
    };
  }

  /**
   * Puts the phone back on the desktop it was talking to before a failed switch.
   *
   * There is one transport, so `prepareAccountDeviceConnection` has to tear the
   * old link down before dialling the new one. When the dial fails, what is
   * left is no device at all, with polling and the heartbeat stopped — tapping
   * a session on a machine that has gone away costs the user the machine they
   * were actually using. The restore is best effort and deliberately silent:
   * the original failure is still what gets reported, because the thing the
   * user asked for did not happen either way. The re-entry guard is what stops
   * a restore that fails from trying to restore itself.
   */
  private async restoreControlTarget(previous?: CloudAccountDevice): Promise<void> {
    if (!previous || this.restoringControlTarget) {
      return;
    }
    this.restoringControlTarget = true;
    try {
      await this.selectCloudAccountDevice(previous, false);
      RemoteLogger.info(`control target restored device=${previous.deviceId}`);
    } catch (err) {
      RemoteLogger.warn(`control target restore failed device=${previous.deviceId}: ${String(err)}`);
    } finally {
      this.restoringControlTarget = false;
    }
  }

  private update(
    apiUrl: string,
    apiKey: string,
    modelName: string,
    clearApiKey: boolean
  ): GeneralChatConfigUpdate {
    return { apiUrl, apiKey, modelName, clearApiKey };
  }

  private async validate(update: GeneralChatConfigUpdate): Promise<string> {
    const snapshot = await this.store.snapshot();
    return GeneralChatConfigValidator.validate(update, snapshot.hasApiKey);
  }

  private async probe(update: GeneralChatConfigUpdate): Promise<string> {
    const apiKey = await this.effectiveApiKey(update);
    if (apiKey.length === 0) {
      return RemoteI18n.t('settings.modelService.apiKeyRequired');
    }
    try {
      await this.hooks.probeConfiguration(update.apiUrl, apiKey, update.modelName);
      return '';
    } catch (err) {
      return ConnectionErrorPolicy.errorText(err);
    }
  }

  private async effectiveApiKey(update: GeneralChatConfigUpdate): Promise<string> {
    const directKey = update.apiKey.trim();
    if (directKey.length > 0) {
      return directKey;
    }
    if (update.clearApiKey) {
      return '';
    }
    return (await this.store.accessToken()).trim();
  }

  private async restoreCloudAccountSession(): Promise<void> {
    const cloud = this.requireCloud();
    try {
      const persisted = await cloud.sessionStore.load();
      if (!persisted) {
        return;
      }
      const session: CloudAccountSession = {
        token: persisted.token,
        userId: persisted.userId,
        masterKey: Encoding.base64ToBytes(persisted.masterKey)
      };
      this.applyCloudAccountSession(session, persisted.relayUrl, persisted.username || session.userId);
      this.restorePersistedControlTarget(persisted);
      await this.loadGeneralChatAccountModels(session, persisted.relayUrl);
    } catch (err) {
      RemoteLogger.warn(`cloud account restore failed: ${err instanceof Error ? err.message : 'unknown error'}`);
      await cloud.sessionStore.clear();
    }
  }

  /**
   * Names the desktop this account was last driving, without connecting to it.
   *
   * The account path has no pairing snapshot on disk the way the scanned-room
   * path does — the target rides along in the encrypted account record — so
   * without this a cold start could not even say which desktop it used to be
   * on, and the restore that follows would have nothing to aim at.
   */
  private restorePersistedControlTarget(persisted: PersistedCloudAccountSession): void {
    const targetId = (persisted.targetDeviceId || '').trim();
    if (targetId.length === 0) {
      return;
    }
    const targetName = (persisted.targetDeviceName || '').trim() || targetId;
    const remoteState = this.requireCloud().remoteState;
    remoteState.setControlTarget('account_device', targetId, targetName);
    remoteState.setDesktopIdentity(targetName, targetId);
  }

  private async loadGeneralChatAccountModels(session: CloudAccountSession, relayUrl: string): Promise<void> {
    const cloud = this.requireCloud();
    this.store.replaceAccountModels([]);
    try {
      const blob = await cloud.client.fetchSettings(relayUrl, session);
      if (!blob) {
        this.store.replaceAccountModels([]);
        await this.refreshModelCatalog();
        RemoteLogger.info('cloud model catalog is empty');
        return;
      }
      const models = GeneralChatCloudConfigPolicy.models(blob.plaintext);
      this.store.replaceAccountModels(models);
      await this.refreshModelCatalog();
      RemoteLogger.info(`cloud model catalog loaded count=${models.length} version=${blob.version}`);
    } catch (err) {
      await this.refreshModelCatalog();
      RemoteLogger.warn(`cloud model catalog load failed: ${err instanceof Error ? err.message : 'unknown error'}`);
    }
  }

  private async expireCloudAccountSession(): Promise<void> {
    const cloud = this.requireCloud();
    cloud.hooks.invalidatePreview();
    if (cloud.remoteState.controlTargetType === 'account_device') {
      this.resetAccountDeviceConnection(true);
    }
    // Publish the signed-out projection before awaiting storage. A revoked
    // credential must never leave stale account devices actionable merely
    // because preferences or cache cleanup is slow (or fails).
    this.clearCloudAccountProjection(cloud);
    this.store.replaceAccountModels([]);
    await this.refreshModelCatalog();
    await cloud.sessionStore.clear();
    await cloud.hooks.clearCachedRemoteData();
    RemoteLogger.info('cloud account session expired');
  }

  /** Drops every observable fact owned by the account session. */
  private clearCloudAccountProjection(cloud: CloudAccountSettingsDependencies): void {
    this.stopPresencePolling();
    this.cloudSession = undefined;
    this.cloudRelayUrl = '';
    this.accountDevices = [];
    this.lastAccountDevicesAt = 0;
    cloud.hooks.onAccountDevices([]);
    cloud.remoteState.setAccountUserId('');
    cloud.remoteState.setAccountUsername('');
  }

  private prepareAccountDeviceConnection(): void {
    const cloud = this.requireCloud();
    cloud.hooks.onBeforeControlTargetChange();
    cloud.hooks.invalidatePreview();
    cloud.hooks.invalidateRemoteActivity();
    cloud.hooks.invalidateRemoteConnection();
    cloud.hooks.stopPolling();
    cloud.hooks.stopHeartbeat();
    cloud.remoteState.setConnectionState('reconnecting');
    cloud.remoteState.setLoadingHome(true);
    cloud.remoteState.clearControlTarget();
    cloud.remoteState.setWorkspace(RemoteI18n.t('status.notConnected'), '', '', '', 'normal');
    cloud.remoteState.setBusy(true);
    cloud.remoteState.setStatusText(RemoteI18n.t('remote.settings.deviceConnecting'));
    cloud.remoteState.clearActiveSession();
    cloud.hooks.resetTimeline();
    cloud.hooks.resetKnownRemoteState();
    cloud.remoteState.setSessions([], false);
  }

  private resetAccountDeviceConnection(clearWorkspace: boolean): void {
    const cloud = this.requireCloud();
    cloud.hooks.invalidateRemoteActivity();
    cloud.hooks.stopPolling();
    cloud.hooks.stopHeartbeat();
    cloud.sessionManager.reset();
    cloud.remoteState.clearActiveSession();
    cloud.remoteState.setSessions([], false);
    if (clearWorkspace) {
      cloud.remoteState.setWorkspace(RemoteI18n.t('status.notConnected'), '', '', '', 'normal');
      cloud.remoteState.setAuthenticatedUserId('');
    }
    cloud.remoteState.clearControlTarget();
    cloud.remoteState.setConnectionState('disconnected');
  }

  private requireCloud(): CloudAccountSettingsDependencies {
    if (!this.cloud) {
      throw new Error('Cloud account settings dependencies are not configured.');
    }
    return this.cloud;
  }
}
