import { createEmbeddedConfig } from './embedded-driver';

export const config = createEmbeddedConfig(
  [
    '../specs/l1-ui-navigation.spec.ts',
    '../specs/l1-workspace.spec.ts',
    '../specs/l1-chat-input.spec.ts',
    '../specs/l1-navigation.spec.ts',
    '../specs/l1-file-tree.spec.ts',
    '../specs/l1-editor.spec.ts',
    '../specs/l1-terminal.spec.ts',
    '../specs/l1-settings.spec.ts',
    '../specs/l1-session.spec.ts',
    '../specs/l1-dialog.spec.ts',
    '../specs/l1-chat.spec.ts',
  ],
  'L1'
);

export default config;
