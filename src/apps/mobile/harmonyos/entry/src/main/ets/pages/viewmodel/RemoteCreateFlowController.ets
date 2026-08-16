import { RecentWorkspaceEntry, RemoteSession, SessionSummary } from '../../model/RemoteModels';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteUiState } from '../../services/RemoteUiState';
import { AppRoute } from '../navigation/AppRouteContract';
import { RemoteCreateSessionState } from '../state/RemoteCreateSessionState';
import { RemotePageState } from '../state/RemotePageState';
import {
  RemoteConversationDependencies,
  requireRemoteRuntime,
  routeCreatedRemoteSession,
  routeRemoteSessionInPlace
} from './ConversationRuntime';

export class RemoteCreateFlowController {
  private readonly remote: RemotePageState;
  private readonly remoteCreate: RemoteCreateSessionState;
  private readonly remoteRuntime?: RemoteConversationDependencies;
  private remoteCreateWorkspaceLoadVersion: number = 0;

  constructor(
    remote: RemotePageState,
    remoteCreate: RemoteCreateSessionState,
    remoteRuntime: RemoteConversationDependencies | undefined
  ) {
    this.remote = remote;
    this.remoteCreate = remoteCreate;
    this.remoteRuntime = remoteRuntime;
  }

  private navigateCreated(sessionId: string): void {
    routeCreatedRemoteSession(sessionId, requireRemoteRuntime(this.remoteRuntime), (dismissed: boolean) => {
      this.remote.setConversationDismissed(dismissed);
    });
  }

  private navigateInPlace(sessionId: string): void {
    routeRemoteSessionInPlace(sessionId, requireRemoteRuntime(this.remoteRuntime), (dismissed: boolean) => {
      this.remote.setConversationDismissed(dismissed);
    });
  }

  async createRemoteSession(agentType: string, inPlace: boolean = false): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    runtime.filePreview.close();
    await runtime.sessions.createSession(
      agentType,
      '',
      inPlace ? (sessionId: string): void => this.navigateInPlace(sessionId) :
        (sessionId: string): void => this.navigateCreated(sessionId)
    );
  }

  openRemoteCreateSession(): void {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (!runtime.connection.ensureAvailable()) {
      return;
    }
    const deviceId = this.remote.controlTargetDeviceId || this.remote.desktopId;
    const deviceName = this.remote.controlTargetDeviceName || this.remote.desktopName;
    this.remoteCreate.prepare(deviceId, deviceName, this.remote.selectedModelId);
    if (deviceId.length > 0) {
      this.remoteCreate.setDevices([{
        deviceId,
        deviceName: deviceName || deviceId,
        online: true
      }]);
    }
    this.remoteCreate.setWorkspaces(this.remote.recentWorkspaces);
    runtime.appShell.pushRoute(AppRoute.RemoteCreate);
    this.loadRemoteCreateChoices();
    this.loadRemoteCreateModelCatalog();
  }

  closeRemoteCreateSession(): void {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    this.remoteCreateWorkspaceLoadVersion += 1;
    runtime.hooks.stopVoiceInput();
    this.remoteCreate.closeMenu();
    runtime.appShell.popRoute(AppRoute.RemoteHome);
  }

  async loadRemoteCreateChoices(): Promise<void> {
    await Promise.all([
      this.loadRemoteCreateDevices(),
      this.loadRemoteCreateWorkspaces()
    ]);
  }

  async loadRemoteCreateModelCatalog(): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (this.remote.modelCatalog.models.length > 0) {
      return;
    }
    try {
      const catalog = await runtime.sessionManager.getModelCatalog();
      const selectedModelId = RemoteUiState.selectedModelIdForCatalog(catalog, this.remote.selectedModelId);
      this.remote.setModelCatalog(catalog, selectedModelId);
      this.remoteCreate.setSelectedModelId(selectedModelId);
    } catch (_err) {
      // Model selection remains hidden when the remote does not expose a catalog.
    }
  }

  async loadRemoteCreateDevices(): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    this.remoteCreate.isLoadingDevices = this.remoteCreate.devices.length === 0;
    try {
      const phoneDeviceId = runtime.connection.getDeviceId();
      const accountDevices = await runtime.settings.listCloudAccountDevices();
      const devices = accountDevices.filter((device: CloudAccountDevice): boolean =>
        device.online && device.deviceId !== phoneDeviceId
      );
      const currentId = this.remoteCreate.selectedDeviceId;
      if (currentId.length > 0 &&
        !devices.some((device: CloudAccountDevice): boolean => device.deviceId === currentId)) {
        devices.unshift({
          deviceId: currentId,
          deviceName: this.remoteCreate.selectedDeviceName || currentId,
          online: true
        });
      }
      this.remoteCreate.setDevices(devices);
    } catch (_err) {
      const currentId = this.remoteCreate.selectedDeviceId;
      if (currentId.length > 0) {
        this.remoteCreate.setDevices([{
          deviceId: currentId,
          deviceName: this.remoteCreate.selectedDeviceName || currentId,
          online: true
        }]);
      } else {
        this.remoteCreate.setDevices([]);
      }
      this.remoteCreate.errorText = RemoteI18n.t('remote.create.deviceLoadFailed');
    }
  }

  async loadRemoteCreateWorkspaces(): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    const loadVersion = ++this.remoteCreateWorkspaceLoadVersion;
    const deviceId = this.remoteCreate.selectedDeviceId;
    this.remoteCreate.isLoadingWorkspaces = this.remoteCreate.workspaces.length === 0;
    try {
      const workspaces = await runtime.workspace.recentWorkspaces();
      if (loadVersion !== this.remoteCreateWorkspaceLoadVersion ||
        deviceId !== this.remoteCreate.selectedDeviceId) {
        return;
      }
      this.remoteCreate.setWorkspaces(workspaces);
    } catch (_err) {
      if (loadVersion !== this.remoteCreateWorkspaceLoadVersion ||
        deviceId !== this.remoteCreate.selectedDeviceId) {
        return;
      }
      this.remoteCreate.setWorkspaces([]);
      this.remoteCreate.errorText = RemoteI18n.t('remote.create.workspaceLoadFailed');
    }
  }

  toggleRemoteCreateDevices(): void {
    this.remoteCreate.toggleMenu('devices');
    if (this.remoteCreate.openMenu === 'devices' && this.remoteCreate.devices.length === 0) {
      this.loadRemoteCreateDevices();
    }
  }

  toggleRemoteCreateWorkspaces(): void {
    this.remoteCreate.toggleMenu('workspaces');
    if (this.remoteCreate.openMenu === 'workspaces' && this.remoteCreate.workspaces.length === 0) {
      this.loadRemoteCreateWorkspaces();
    }
  }

  async selectRemoteCreateDevice(device: CloudAccountDevice): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (device.deviceId === this.remoteCreate.selectedDeviceId) {
      this.remoteCreate.closeMenu();
      return;
    }
    const draft = this.remoteCreate.draft;
    this.remoteCreate.closeMenu();
    this.remoteCreate.isLoadingWorkspaces = true;
    try {
      await runtime.settings.selectCloudAccountDevice(device, false);
      this.remoteCreate.selectDevice(device);
      this.remoteCreate.setDraft(draft);
      await this.loadRemoteCreateWorkspaces();
    } catch (err) {
      this.remoteCreate.isLoadingWorkspaces = false;
      this.remoteCreate.errorText = err instanceof Error ? err.message :
        RemoteI18n.t('remote.settings.deviceSwitchFailed');
    }
  }

  selectRemoteCreateWorkspace(path: string): void {
    const workspace = this.remoteCreate.workspaces
      .find((item: RecentWorkspaceEntry): boolean => item.path === path);
    this.remoteCreate.selectWorkspace(workspace);
  }

  async submitRemoteCreateSession(): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    const instruction = this.remoteCreate.draft.trim();
    if (instruction.length === 0 || this.remoteCreate.isSubmitting || !runtime.connection.ensureAvailable()) {
      return;
    }
    const context = this.remoteCreate.submissionContext();
    const activeDeviceId = this.remote.controlTargetDeviceId || this.remote.desktopId;
    if (context.deviceId.length === 0 || context.deviceId !== activeDeviceId) {
      this.remoteCreate.errorText = RemoteI18n.t('remote.create.deviceMismatch');
      return;
    }
    this.remoteCreate.isSubmitting = true;
    this.remoteCreate.errorText = '';
    this.remoteCreate.closeMenu();
    try {
      if (context.workspacePath.length > 0) {
        await runtime.sessions.createSessionInWorkspace(
          context.workspacePath,
          this.remote.workspacePath,
          instruction,
          context.agentType,
          undefined,
          this.remoteCreate.selectedModelId
        );
      } else {
        await this.bindAssistantWorkspace();
        await runtime.sessions.createSession(
          context.agentType,
          instruction,
          undefined,
          this.remoteCreate.selectedModelId
        );
      }
      if (runtime.appShell.isRoute(AppRoute.RemoteCreate)) {
        this.remoteCreate.errorText = this.remote.statusText || RemoteI18n.t('remote.create.submitFailed');
      }
    } catch (err) {
      this.remoteCreate.errorText = err instanceof Error ? err.message :
        RemoteI18n.t('remote.create.submitFailed');
    } finally {
      this.remoteCreate.isSubmitting = false;
    }
  }

  /**
   * The chat option creates a Claw session, and the desktop always binds those
   * to its assistant workspace. Follow it there first, otherwise the app stays
   * bound to the code workspace it was on and the new chat is listed, titled
   * and file-scoped as if it had been created inside that workspace.
   */
  private async bindAssistantWorkspace(): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (this.remote.workspaceKind === 'assistant') {
      return;
    }
    try {
      const assistants = await runtime.workspace.assistants();
      if (assistants.length === 0) {
        return;
      }
      await runtime.hooks.selectAssistantWorkspace(assistants[0].path);
    } catch (err) {
      RemoteLogger.warn(`assistant workspace bind failed: ${String(err)}`);
    }
  }

  async createRemoteSessionInWorkspace(
    path: string,
    agentType: string = 'code',
    inPlace: boolean = false
  ): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    runtime.filePreview.close();
    await runtime.sessions.createSessionInWorkspace(
      path,
      this.remote.workspacePath,
      '',
      agentType,
      inPlace ? (sessionId: string): void => this.navigateInPlace(sessionId) :
        (sessionId: string): void => this.navigateCreated(sessionId)
    );
  }

  async openRemoteSession(item: RemoteSession, inPlace: boolean = false): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    runtime.filePreview.close();
    await runtime.sessions.openSession(
      item,
      this.remote.workspacePath,
      inPlace ? (sessionId: string): void => this.navigateInPlace(sessionId) :
        (sessionId: string): void => this.navigateCreated(sessionId)
    );
  }

  applyRemoteActiveSession(session: SessionSummary): void {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    const current = this.remote.activeSession;
    if (runtime.hooks.isFilePreviewVisible() &&
      (current.sessionId !== session.sessionId || current.workspacePath !== session.workspacePath)) {
      runtime.filePreview.close();
    }
    this.remote.setActiveSession(session);
  }

  async deleteRemoteSession(item: RemoteSession): Promise<void> {
    await requireRemoteRuntime(this.remoteRuntime).sessions.deleteSession(item, this.remote.workspacePath);
  }

  routeCreatedRemoteSession(sessionId: string): void {
    this.navigateCreated(sessionId);
  }
}

