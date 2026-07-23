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

function buildFormPatch(form) {
  if (!form || typeof form !== 'object') return {};
  return {
    baseUrl: form.baseUrl ?? '',
    apiKey: form.apiKey ?? '',
    apiKeyPlaceholder: form.apiKeyPlaceholder ?? '',
    provider: form.provider ?? '',
    providerPlaceholder: form.providerPlaceholder ?? '',
    modelSelection: form.modelSelection ?? '',
    modelName: form.modelName ?? '',
    resolvedUrlLabel: form.resolvedUrlLabel ?? '',
  };
}

function buildFormatsPatch(formats) {
  if (!formats || typeof formats !== 'object') return {};
  return {
    openaiCompatible: formats.openaiCompatible ?? '',
    responsesApi: formats.responsesApi ?? '',
    claudeApi: formats.claudeApi ?? '',
    geminiApi: formats.geminiApi ?? '',
  };
}

function buildModelPatch(settingsAiModel, languageTag, components) {
  const isZh = languageTag === 'zh';
  const form = get(settingsAiModel, 'form', {});
  const formats = get(settingsAiModel, 'formats', {});
  const input = get(components, 'input', {});
  return {
    description: get(
      settingsAiModel,
      'subtitle',
      'Configure AI model provider, API key, and advanced parameters.'
    ),
    providerLabel: get(settingsAiModel, 'providerSelection.title', 'Model Provider'),
    selectProvider: get(settingsAiModel, 'providerSelection.orSelectProvider', 'Select a provider...'),
    customProvider: get(settingsAiModel, 'providerSelection.customTitle', 'Custom'),
    getApiKey: get(settingsAiModel, 'providerSelection.getApiKey', 'How to get an API Key?'),
    fillApiKeyBeforeFetch: get(
      settingsAiModel,
      'providerSelection.fillApiKeyBeforeFetch',
      'Enter the API key before fetching models'
    ),
    fetchingModels: get(settingsAiModel, 'providerSelection.fetchingModels', 'Fetching model list...'),
    fetchFailedFallback: get(
      settingsAiModel,
      'providerSelection.fetchFailedFallback',
      'Failed to fetch model list, fell back to common preset models'
    ),
    fetchEmptyFallback: get(
      settingsAiModel,
      'providerSelection.fetchEmptyFallback',
      'Provider returned no models, fell back to common preset models'
    ),
    usingPresetModels: get(
      settingsAiModel,
      'providerSelection.usingPresetModels',
      'Currently showing common preset models'
    ),
    modelNamePlaceholder: get(
      settingsAiModel,
      'providerSelection.inputModelName',
      get(settingsAiModel, 'form.modelName', 'Enter model name...')
    ),
    modelNameSelectPlaceholder: get(settingsAiModel, 'providerSelection.selectModel', 'Select a model...'),
    modelNoResults: isZh ? '没有匹配的模型' : 'No matching models',
    /** Installer: use addCustomModel (not useCustomModel / "Press Enter") for the extra dropdown option */
    addCustomModel: get(settingsAiModel, 'providerSelection.addCustomModel', 'Add Custom Model'),
    form: buildFormPatch(form),
    formats: buildFormatsPatch(formats),
    showSecret: get(input, 'show', 'Show'),
    hideSecret: get(input, 'hide', 'Hide'),
    baseUrlPlaceholder: isZh
      ? '示例：https://open.bigmodel.cn/api/paas/v4/chat/completions'
      : 'e.g., https://open.bigmodel.cn/api/paas/v4/chat/completions',
    customRequestBodyPlaceholder: get(
      settingsAiModel,
      'advancedSettings.customRequestBody.placeholder',
      '{\n  "temperature": 0.8,\n  "top_p": 0.9\n}'
    ),
    jsonValid: get(settingsAiModel, 'advancedSettings.customRequestBody.validJson', 'Valid JSON format'),
    jsonInvalid: get(
      settingsAiModel,
      'advancedSettings.customRequestBody.invalidJson',
      'Invalid JSON format'
    ),
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
    testConnection: get(settingsAiModel, 'actions.test', 'Test Connection'),
    testing: isZh ? '测试中...' : 'Testing...',
    testSuccess: get(settingsAiModel, 'messages.testSuccess', 'Connection successful'),
    testFailed: get(settingsAiModel, 'messages.testFailed', 'Connection failed'),
    advancedShow: 'Show advanced settings',
    advancedHide: 'Hide advanced settings',
    providers: buildProviderPatch(settingsAiModel),
  };
}

function syncOne(languageTag) {
  const localeDir = languageTag === 'zh' ? 'zh-CN' : 'en-US';
  const installerLocale = languageTag === 'zh' ? 'zh.json' : 'en.json';

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
  const sourceComponentsPath = path.join(
    PROJECT_ROOT,
    'src',
    'web-ui',
    'src',
    'locales',
    localeDir,
    'components.json'
  );

  const targetPath = path.join(INSTALLER_ROOT, 'src', 'i18n', 'locales', installerLocale);

  const settingsAiModel = readJson(sourceAiModelPath);
  let components = {};
  try {
    components = readJson(sourceComponentsPath);
  } catch {
    // optional
  }
  const target = readJson(targetPath);

  const patch = buildModelPatch(settingsAiModel, languageTag, components);
  target.model = mergeDeep(target.model || {}, patch);

  writeJson(targetPath, target);
}

function main() {
  syncOne('en');
  syncOne('zh');
  console.log('[sync-model-i18n] Synced installer model i18n from web-ui locales.');
}

main();
