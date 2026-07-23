import { createEmbeddedConfig } from './embedded-driver';

export const config = createEmbeddedConfig(
  [
    '../specs/l0-smoke.spec.ts',
    '../specs/l0-webdriver-protocol.spec.ts',
    '../specs/l0-open-workspace.spec.ts',
    '../specs/l0-open-settings.spec.ts',
    '../specs/l0-observe.spec.ts',
    '../specs/l0-navigation.spec.ts',
    '../specs/l0-tabs.spec.ts',
    '../specs/l0-theme.spec.ts',
    '../specs/l0-i18n.spec.ts',
    '../specs/l0-notification.spec.ts',
  ],
  'L0'
);

export default config;
