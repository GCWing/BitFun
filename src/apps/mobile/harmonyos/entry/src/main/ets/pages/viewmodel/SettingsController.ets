import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CloudAccountDevice, CloudAccountRequestError, CloudAccountSession, CloudAccountClient } from '../../services/CloudAccountClient';
import { CloudAccountSessionStore } from '../../services/CloudAccountSessionStore';
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
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteSessionManager } from '../../services/RemoteSessionManager';
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
  readonly closeSettings: () => void;
  readonly closeConnectSheet: () => void;
  readonly navigateRemoteHome: () => void;
  readonly loadRecentWorkspaces: () => Promise<void>;
}

export interface CloudAccountSettingsDependencies {
  readonly client: CloudAccountClient;
  readonly sessionStore: CloudAccountSessionStore;
  readonly sessionManager: RemoteSessionManager;
  readonly remoteState: RemotePageState;
  readonly hooks: CloudAccountSettingsHooks;
}

/** Owns general-chat model service settings and their presentation projection. */
export class SettingsController {
  private readonly store: GeneralChatConfigStore;
  private readonly state: GeneralChatPageState;
  private readonly hooks: SettingsControllerHooks;
  private readonly cloud?: CloudAccountSettingsDependencies;
  private cloudSession?: CloudAccountSession;
  private cloudRelayUrl: string = '';

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

  async syncCloudAccount(): Promise<string> {
    const cloud = this.requireCloud();
    const session = this.cloudSession;
    if (!session || this.cloudRelayUrl.length === 0) {
      throw new Error(RemoteI18n.t('remote.settings.accountNotSignedIn'));
    }
    let bundles: Object[];
    try {
      bundles = await cloud.client.fetchSessions(this.cloudRelayUrl, session, 0);
    } catch (err) {
      if (err instanceof CloudAccountRequestError && err.statusCode === 401) {
        await this.expireCloudAccountSession();
        throw new Error(RemoteI18n.t('remote.settings.accountExpired'));
      }
      throw new Error(err instanceof Error ? err.message : RemoteI18n.t('remote.settings.accountSyncFailed'));
    }
    await this.loadGeneralChatAccountModels(session, this.cloudRelayUrl);
    RemoteLogger.info(`cloud account backup sync completed count=${bundles.length}`);
    return String(bundles.length);
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
    this.cloudSession = undefined;
    this.cloudRelayUrl = '';
    this.store.replaceAccountModels([]);
    await this.refreshModelCatalog();
    await cloud.sessionStore.clear();
    cloud.remoteState.setAccountUserId('');
    cloud.remoteState.setAccountUsername('');
    cloud.remoteState.clearControlTarget();
    RemoteLogger.info('cloud account logout success');
  }

  async listCloudAccountDevices(): Promise<CloudAccountDevice[]> {
    const cloud = this.requireCloud();
    const session = this.cloudSession;
    if (!session || this.cloudRelayUrl.length === 0) {
      return [];
    }
    try {
      return await cloud.client.listDevices(this.cloudRelayUrl, session);
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

  async restoreCloudTarget(targetDeviceId: string, targetDeviceName: string): Promise<void> {
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
      });
    } catch (err) {
      RemoteLogger.warn(`cloud target restore failed: ${err instanceof Error ? err.message : 'unknown error'}`);
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
      cloud.remoteState.setSessions(initialSync.sessions, initialSync.hasMoreSessions);
      cloud.remoteState.setAuthenticatedUserId(initialSync.authenticatedUserId);
      cloud.remoteState.setConnectionState('connected');
      cloud.remoteState.setStatusText(RemoteI18n.t('connection.connected'));
      cloud.hooks.closeSettings();
      cloud.hooks.closeConnectSheet();
      if (navigateHome) {
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
      await cloud.hooks.loadRecentWorkspaces();
    } catch (err) {
      if (err instanceof CloudAccountRequestError && err.statusCode === 401) {
        await this.expireCloudAccountSession();
      }
      cloud.remoteState.clearControlTarget();
      cloud.remoteState.setConnectionState('failed');
      const message = ConnectionErrorPolicy.errorText(err);
      cloud.remoteState.setStatusText(message);
      cloud.sessionManager.reset();
      throw new Error(message);
    } finally {
      cloud.remoteState.setLoadingHome(false);
      cloud.remoteState.setBusy(false);
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
      await this.loadGeneralChatAccountModels(session, persisted.relayUrl);
    } catch (err) {
      RemoteLogger.warn(`cloud account restore failed: ${err instanceof Error ? err.message : 'unknown error'}`);
      await cloud.sessionStore.clear();
    }
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
    this.cloudSession = undefined;
    this.cloudRelayUrl = '';
    await cloud.sessionStore.clear();
    cloud.remoteState.setAccountUserId('');
    cloud.remoteState.setAccountUsername('');
    if (cloud.remoteState.controlTargetType === 'account_device') {
      this.resetAccountDeviceConnection(false);
    }
  }

  private prepareAccountDeviceConnection(): void {
    const cloud = this.requireCloud();
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
