import { appTasks } from '@ohos/hvigor-ohos-plugin';
import { existsSync, readFileSync } from 'fs';
import { dirname, resolve } from 'path';

interface LocalSigningFile {
  useDefaultDebug?: boolean;
  material?: Object;
  type?: string;
}

interface LocalProjectOverrides {
  ohos?: Object;
}

function loadLocalOverrides(): LocalProjectOverrides {
  let currentDir = process.cwd();
  const signingConfigCandidates: string[] = [
    resolve(currentDir, 'signing.local.json5'),
    resolve(currentDir, 'src/apps/mobile/harmonyos/signing.local.json5'),
    resolve(__dirname, 'signing.local.json5')
  ];

  while (dirname(currentDir) !== currentDir) {
    signingConfigCandidates.push(resolve(currentDir, 'signing.local.json5'));
    currentDir = dirname(currentDir);
  }

  const signingConfigPath = signingConfigCandidates.find((candidate) => existsSync(candidate));
  if (signingConfigPath === undefined) {
    return {};
  }

  const signingConfig = JSON.parse(readFileSync(signingConfigPath, 'utf8')) as LocalSigningFile;
  if (signingConfig.useDefaultDebug === true) {
    throw new Error('useDefaultDebug is not supported by this Hvigor version. Provide material in signing.local.json5.');
  }

  if (signingConfig.material === undefined) {
    return {};
  }

  if (signingConfig.material !== undefined) {
    return {
      ohos: {
        overrides: {
          signingConfig
        }
      }
    };
  }

  return {};
}

function applyLocalOverrides(localOverrides: LocalProjectOverrides): void {
  if (localOverrides.ohos === undefined) {
    return;
  }

  const projectConfigManagerModule = require('@ohos/hvigor-ohos-plugin/src/common/global/project-ohos-config-manager.js');
  projectConfigManagerModule.projectOhosConfigManager.loaderConfig(localOverrides.ohos);
}

const localOverrides = loadLocalOverrides();
applyLocalOverrides(localOverrides);

export default {
  system: appTasks, /* Built-in plugin of Hvigor. It cannot be modified. */
  plugins: []       /* Custom plugin to extend the functionality of Hvigor. */
}
