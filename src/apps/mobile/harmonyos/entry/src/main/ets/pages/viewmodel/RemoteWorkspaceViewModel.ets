import { RecentWorkspaceEntry, WorkspaceInfo } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteWorkspaceCoordinator } from '../../services/RemoteWorkspaceCoordinator';
import { RemotePageState } from '../state/RemotePageState';

export interface RemoteWorkspaceViewModelHooks {
  readonly isRemoteAvailable: () => boolean;
  readonly isBusy: () => boolean;
  readonly onBusy: (isBusy: boolean) => void;
  readonly onStatus: (statusText: string) => void;
  readonly onWorkspaceSelected: (workspace: WorkspaceInfo) => void;
  readonly onRefreshSessions: () => Promise<void>;
  readonly onConnectionFailure: (error: Object) => void;
}

/** Owns the workspace/assistant picker workflows and their presentation state. */
export class RemoteWorkspaceViewModel {
  private readonly pageState: RemotePageState;
  private readonly coordinator: RemoteWorkspaceCoordinator;
  private readonly hooks: RemoteWorkspaceViewModelHooks;

  constructor(
    pageState: RemotePageState,
    coordinator: RemoteWorkspaceCoordinator,
    hooks: RemoteWorkspaceViewModelHooks
  ) {
    this.pageState = pageState;
    this.coordinator = coordinator;
    this.hooks = hooks;
  }

  async toggleRecentWorkspaces(): Promise<void> {
    if (this.pageState.showWorkspacePicker) {
      this.pageState.setWorkspacePickerVisible(false);
      return;
    }
    this.pageState.setWorkspacePickerVisible(true);
    this.pageState.setAssistantPickerVisible(false);
    await this.loadRecentWorkspaces();
  }

  async toggleAssistants(): Promise<void> {
    if (this.pageState.showAssistantPicker) {
      this.pageState.setAssistantPickerVisible(false);
      return;
    }
    this.pageState.setAssistantPickerVisible(true);
    this.pageState.setWorkspacePickerVisible(false);
    await this.loadAssistants();
  }

  async selectWorkspace(path: string): Promise<boolean> {
    return await this.select(path, false);
  }

  async selectAssistant(path: string): Promise<boolean> {
    return await this.select(path, true);
  }

  async loadRecentWorkspacesInBackground(): Promise<void> {
    try {
      const recent = await this.coordinator.recentWorkspaces();
      const assistants = await this.coordinator.assistants();
      const assistantWorkspaces: RecentWorkspaceEntry[] = [];
      assistants.forEach((item) => {
        assistantWorkspaces.push({
          path: item.path,
          name: item.name,
          lastOpened: '',
          workspaceKind: 'assistant'
        } as RecentWorkspaceEntry);
      });
      const allWorkspaces = recent.slice();
      assistantWorkspaces.forEach((item: RecentWorkspaceEntry) => {
        if (!allWorkspaces.some((existing: RecentWorkspaceEntry) => existing.path === item.path)) {
          allWorkspaces.push(item);
        }
      });
      this.pageState.setRecentWorkspaces(allWorkspaces);
    } catch (err) {
      RemoteLogger.warn(`background recent workspace load failed: ${String(err)}`);
    }
  }

  private async loadRecentWorkspaces(): Promise<void> {
    if (!this.hooks.isRemoteAvailable()) {
      return;
    }
    try {
      this.hooks.onBusy(true);
      this.hooks.onStatus(RemoteI18n.t('status.loadingRecentWorkspaces'));
      const recent = await this.coordinator.recentWorkspaces();
      this.pageState.setRecentWorkspaces(recent);
      this.hooks.onStatus(recent.length > 0 ?
        RemoteI18n.t('status.chooseWorkspace') : RemoteI18n.t('status.noRecentWorkspaces'));
    } catch (err) {
      this.hooks.onConnectionFailure(err);
    } finally {
      this.hooks.onBusy(false);
    }
  }

  private async loadAssistants(): Promise<void> {
    if (!this.hooks.isRemoteAvailable()) {
      return;
    }
    try {
      this.hooks.onBusy(true);
      this.hooks.onStatus(RemoteI18n.t('status.loadingAssistants'));
      const assistants = await this.coordinator.assistants();
      this.pageState.setAssistants(assistants);
      this.hooks.onStatus(assistants.length > 0 ?
        RemoteI18n.t('status.chooseAssistant') : RemoteI18n.t('status.noAssistants'));
    } catch (err) {
      this.hooks.onConnectionFailure(err);
    } finally {
      this.hooks.onBusy(false);
    }
  }

  private async select(path: string, assistant: boolean): Promise<boolean> {
    if (this.hooks.isBusy() || path.trim().length === 0 || !this.hooks.isRemoteAvailable()) {
      return false;
    }
    try {
      this.hooks.onBusy(true);
      this.hooks.onStatus(RemoteI18n.t(assistant ?
        'status.switchingAssistant' : 'status.switchingWorkspace'));
      const workspace = assistant ?
        await this.coordinator.switchAssistant(path) : await this.coordinator.switchWorkspace(path);
      this.hooks.onWorkspaceSelected(workspace);
      this.pageState.closePickers();
      this.pageState.clear();
      this.hooks.onStatus(RemoteI18n.t(assistant ?
        'status.assistantSwitched' : 'status.workspaceSwitched'));
      this.hooks.onBusy(false);
      await this.hooks.onRefreshSessions();
      return true;
    } catch (err) {
      this.hooks.onConnectionFailure(err);
      this.hooks.onBusy(false);
      return false;
    }
  }
}
