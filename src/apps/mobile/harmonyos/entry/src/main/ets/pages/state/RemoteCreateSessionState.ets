import { RecentWorkspaceEntry } from '../../model/RemoteModels';
import { CloudAccountDevice } from '../../services/CloudAccountClient';

export class RemoteCreateSessionContext {
  readonly deviceId: string;
  readonly workspacePath: string;
  readonly agentType: string;

  constructor(deviceId: string, workspacePath: string, agentType: string = 'Claw') {
    this.deviceId = deviceId;
    this.workspacePath = workspacePath;
    this.agentType = agentType;
  }
}

@ObservedV2
export class RemoteCreateSessionState {
  @Trace draft: string = '';
  @Trace devices: CloudAccountDevice[] = [];
  @Trace workspaces: RecentWorkspaceEntry[] = [];
  @Trace selectedDeviceId: string = '';
  @Trace selectedDeviceName: string = '';
  @Trace selectedWorkspacePath: string = '';
  @Trace selectedWorkspaceName: string = '';
  @Trace openMenu: string = 'none';
  @Trace isLoadingDevices: boolean = false;
  @Trace isLoadingWorkspaces: boolean = false;
  @Trace isSubmitting: boolean = false;
  @Trace isVoiceListening: boolean = false;
  @Trace selectedModelId: string = '';
  @Trace errorText: string = '';

  prepare(deviceId: string, deviceName: string, selectedModelId: string = ''): void {
    this.draft = '';
    this.devices = [];
    this.workspaces = [];
    this.selectedDeviceId = deviceId;
    this.selectedDeviceName = deviceName;
    this.selectedWorkspacePath = '';
    this.selectedWorkspaceName = '';
    this.openMenu = 'none';
    this.isLoadingDevices = false;
    this.isLoadingWorkspaces = false;
    this.isSubmitting = false;
    this.isVoiceListening = false;
    this.selectedModelId = selectedModelId;
    this.errorText = '';
  }

  setDraft(draft: string): void {
    this.draft = draft;
    this.errorText = '';
  }

  setSelectedModelId(modelId: string): void {
    this.selectedModelId = modelId;
    this.errorText = '';
  }

  setDevices(devices: CloudAccountDevice[]): void {
    this.devices = devices.slice();
    const selected = devices.find((device: CloudAccountDevice): boolean =>
      device.deviceId === this.selectedDeviceId
    );
    if (selected) {
      this.selectedDeviceName = selected.deviceName;
    }
    this.isLoadingDevices = false;
  }

  setWorkspaces(workspaces: RecentWorkspaceEntry[]): void {
    this.workspaces = workspaces.slice();
    const selected = workspaces.find((workspace: RecentWorkspaceEntry): boolean =>
      workspace.path === this.selectedWorkspacePath
    );
    if (selected) {
      this.selectedWorkspaceName = selected.name;
    }
    this.isLoadingWorkspaces = false;
  }

  selectDevice(device: CloudAccountDevice): void {
    this.selectedDeviceId = device.deviceId;
    this.selectedDeviceName = device.deviceName;
    this.clearWorkspace();
    this.openMenu = 'none';
    this.errorText = '';
  }

  selectWorkspace(workspace?: RecentWorkspaceEntry): void {
    this.selectedWorkspacePath = workspace?.path || '';
    this.selectedWorkspaceName = workspace?.name || '';
    this.openMenu = 'none';
    this.errorText = '';
  }

  submissionContext(): RemoteCreateSessionContext {
    return new RemoteCreateSessionContext(this.selectedDeviceId, this.selectedWorkspacePath);
  }

  clearWorkspace(): void {
    this.selectedWorkspacePath = '';
    this.selectedWorkspaceName = '';
  }

  toggleMenu(menu: string): void {
    this.openMenu = this.openMenu === menu ? 'none' : menu;
  }

  closeMenu(): void {
    this.openMenu = 'none';
  }
}
