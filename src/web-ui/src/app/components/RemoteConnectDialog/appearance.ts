import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const remoteConnectDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'remote-connect-dialog',
  parts: [
    { id: 'root' },
    { id: 'groups' },
    { id: 'groupTab' },
    { id: 'subtabs' },
    { id: 'subtab' },
    { id: 'panel' },
    { id: 'body' },
    { id: 'status' },
    { id: 'error' },
  ],
  facets: [
    { id: 'group', attribute: 'data-bf-group', values: ['network', 'bot', 'account'] },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'connected', selector: { kind: 'self', suffix: '[data-bf-state~="connected"]' } },
  ],
};

export const remoteAccountPanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'remote-account-panel',
  parts: [
    { id: 'root' },
    { id: 'error' },
    { id: 'loading' },
    { id: 'scroll' },
    { id: 'form' },
    { id: 'actions' },
    { id: 'syncOptions' },
    { id: 'syncOption' },
    { id: 'server' },
    { id: 'syncStatus' },
    { id: 'progressTrack' },
    { id: 'progressFill' },
    { id: 'deviceList' },
    { id: 'deviceCard' },
    { id: 'pages' },
    { id: 'pagesEntry' },
  ],
  facets: [
    { id: 'view', attribute: 'data-bf-view', values: ['login', 'overwrite', 'devices'] },
  ],
  states: [
    { id: 'offline', selector: { kind: 'self', suffix: '[data-bf-state~="offline"]' } },
    { id: 'current', selector: { kind: 'self', suffix: '[data-bf-state~="current"]' } },
    { id: 'syncing', selector: { kind: 'self', suffix: '[data-bf-state~="syncing"]' } },
  ],
};
