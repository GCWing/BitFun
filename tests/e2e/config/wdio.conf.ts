import { createEmbeddedConfig } from './embedded-driver';

export const config = createEmbeddedConfig(['../specs/**/*.spec.ts'], 'E2E');

export default config;
