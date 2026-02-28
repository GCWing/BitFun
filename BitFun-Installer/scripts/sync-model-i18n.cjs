const fs = require('fs');
const path = require('path');

const INSTALLER_ROOT = path.resolve(__dirname, '..');
const PROJECT_ROOT = path.resolve(INSTALLER_ROOT, '..');

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function writeJson(filePath, data) {
  fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`, 'utf8');
}

function get(obj, keyPath, fallback) {
  const segments = keyPath.split('.');
  let current = obj;
  for (const seg of segments) {
    if (!current || typeof current !== 'object' || !(seg in current)) {
      return fallback;
    }
    current = current[seg];
  }
  return current ?? fallback;
}

function mergeDeep(target, source) {
  const result = { ...(target || {}) };
  for (const [key, value] of Object.entries(source || {})) {
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      result[key] = mergeDeep(result[key], value);
    } else {
      result[key] = value;
    }
  }
  return result;
}

function buildProviderPatch(settingsAiModel) {
  const providers = get(settingsAiModel, 'providers', {});
  const providerPatch = {};

  for (const [providerId, provider] of Object.entries(providers)) {
    providerPatch[providerId] = {
      name: get(provider, 'name', providerId),
      description: get(provider, 'description', ''),
    };

    if (provider && provider.urlOptions && typeof provider.urlOptions === 'object') {
      providerPatch[providerId].urlOptions = { ...provider.urlOptions };
    }
  }

  return providerPatch;
}

function buildModelPatch(onboarding, settingsAiModel, languageTag) {
  const isZh = languageTag === 'zh';
  return {
    description: get(
      onboarding,
      'model.description',
      'Configure AI model provider, API key, and advanced parameters.'
    ),
    providerLabel: get(onboarding, 'model.provider.label', 'Model Provider'),
    selectProvider: get(onboarding, 'model.provider.placeholder', 'Select a provider...'),
    customProvider: get(onboarding, 'model.provider.options.custom', 'Custom'),
    getApiKey: get(onboarding, 'model.apiKey.help', 'How to get an API Key?'),
    modelNamePlaceholder: get(
      onboarding,
      'model.modelName.inputPlaceholder',
      get(onboarding, 'model.modelName.placeholder', 'Enter model name...')
    ),
    modelNameSelectPlaceholder: get(onboarding, 'model.modelName.selectPlaceholder', 'Select a model...'),
    modelSearchPlaceholder: get(
      onboarding,
      'model.modelName.searchPlaceholder',
      'Search or enter a custom model name...'
    ),
    modelNoResults: isZh ? '没有匹配的模型' : 'No matching models',
    customModel: get(onboarding, 'model.modelName.customHint', 'Use custom model name'),
    baseUrlPlaceholder: get(onboarding, 'model.baseUrl.placeholder', 'Enter API URL'),
    customRequestBodyPlaceholder: get(
      onboarding,
      'model.advanced.customRequestBodyPlaceholder',
      '{\n  "temperature": 0.8,\n  "top_p": 0.9\n}'
    ),
    jsonValid: get(onboarding, 'model.advanced.jsonValid', 'Valid JSON format'),
    jsonInvalid: get(onboarding, 'model.advanced.jsonInvalid', 'Invalid JSON format'),
    skipSslVerify: get(
      settingsAiModel,
      'advancedSettings.skipSslVerify.label',
      'Skip SSL Certificate Verification'
    ),
    customHeadersModeMerge: get(
      settingsAiModel,
      'advancedSettings.customHeaders.modeMerge',
      'Merge Override'
    ),
    customHeadersModeReplace: get(
      settingsAiModel,
      'advancedSettings.customHeaders.modeReplace',
      'Replace All'
    ),
    addHeader: get(settingsAiModel, 'advancedSettings.customHeaders.addHeader', 'Add Field'),
    headerKey: get(settingsAiModel, 'advancedSettings.customHeaders.keyPlaceholder', 'key'),
    headerValue: get(settingsAiModel, 'advancedSettings.customHeaders.valuePlaceholder', 'value'),
    testConnection: get(onboarding, 'model.testConnection', 'Test Connection'),
    testing: get(onboarding, 'model.testing', 'Testing...'),
    testSuccess: get(onboarding, 'model.testSuccess', 'Connection successful'),
    testFailed: get(onboarding, 'model.testFailed', 'Connection failed'),
    advancedShow: 'Show advanced settings',
    advancedHide: 'Hide advanced settings',
    providers: buildProviderPatch(settingsAiModel),
  };
}

function syncOne(languageTag) {
  const localeDir = languageTag === 'zh' ? 'zh-CN' : 'en-US';
  const installerLocale = languageTag === 'zh' ? 'zh.json' : 'en.json';

  const sourceOnboardingPath = path.join(
    PROJECT_ROOT,
    'src',
    'web-ui',
    'src',
    'locales',
    localeDir,
    'onboarding.json'
  );

  const sourceAiModelPath = path.join(
    PROJECT_ROOT,
    'src',
    'web-ui',
    'src',
    'locales',
    localeDir,
    'settings',
    'ai-model.json'
  );

  const targetPath = path.join(INSTALLER_ROOT, 'src', 'i18n', 'locales', installerLocale);

  const onboarding = readJson(sourceOnboardingPath);
  const settingsAiModel = readJson(sourceAiModelPath);
  const target = readJson(targetPath);

  const patch = buildModelPatch(onboarding, settingsAiModel, languageTag);
  target.model = mergeDeep(target.model || {}, patch);

  writeJson(targetPath, target);
}

function main() {
  syncOne('en');
  syncOne('zh');
  console.log('[sync-model-i18n] Synced installer model i18n from web-ui locales.');
}

main();
