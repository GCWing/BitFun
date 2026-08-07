import { GeneralChatRuntimeModelConfig } from './GeneralChatConfigStore';

interface CloudModelAuthWire {
  type?: string;
}

interface CloudModelWire {
  id?: string;
  name?: string;
  provider?: string;
  model_name?: string;
  base_url?: string;
  api_key?: string;
  enabled?: boolean;
  category?: string;
  auth?: CloudModelAuthWire;
}

interface CloudDefaultModelsWire {
  primary?: string;
}

interface CloudAiConfigWire {
  models?: CloudModelWire[];
  default_models?: CloudDefaultModelsWire;
}

interface CloudGlobalConfigWire {
  ai?: CloudAiConfigWire;
}

interface CloudSettingsPayloadWire {
  config?: CloudGlobalConfigWire;
  ai?: CloudAiConfigWire;
}

/** Maps the shared encrypted account settings payload to the smaller HarmonyOS chat contract. */
export class GeneralChatCloudConfigPolicy {
  static models(payload: string): GeneralChatRuntimeModelConfig[] {
    let parsed: CloudSettingsPayloadWire;
    try {
      parsed = JSON.parse(payload) as CloudSettingsPayloadWire;
    } catch (_err) {
      return [];
    }
    const ai = parsed.config?.ai || parsed.ai;
    const models = ai?.models || [];
    const primaryId = ai?.default_models?.primary || '';
    const ordered: CloudModelWire[] = [];
    if (primaryId.length > 0) {
      models.forEach((model: CloudModelWire) => {
        if (model.id === primaryId) {
          GeneralChatCloudConfigPolicy.pushCompatible(ordered, model);
        }
      });
    }
    models.forEach((model: CloudModelWire) => {
      if (model.category === 'general_chat') {
        GeneralChatCloudConfigPolicy.pushCompatible(ordered, model);
      }
    });
    models.forEach((model: CloudModelWire) => {
      GeneralChatCloudConfigPolicy.pushCompatible(ordered, model);
    });
    return ordered.map((model: CloudModelWire): GeneralChatRuntimeModelConfig => {
      const sourceId = GeneralChatCloudConfigPolicy.stringValue(model.id).trim() ||
        `${GeneralChatCloudConfigPolicy.stringValue(model.provider)}:${GeneralChatCloudConfigPolicy.stringValue(model.model_name)}`;
      const modelName = GeneralChatCloudConfigPolicy.stringValue(model.model_name).trim();
      return {
        modelId: `cloud:${sourceId}`,
        name: GeneralChatCloudConfigPolicy.stringValue(model.name).trim() || modelName,
        provider: GeneralChatCloudConfigPolicy.stringValue(model.provider).trim() || 'account',
        apiUrl: GeneralChatCloudConfigPolicy.normalizedApiUrl(model),
        apiKey: GeneralChatCloudConfigPolicy.stringValue(model.api_key).trim(),
        modelName
      };
    });
  }

  static selectModel(payload: string): GeneralChatRuntimeModelConfig | undefined {
    const models = GeneralChatCloudConfigPolicy.models(payload);
    return models.length > 0 ? models[0] : undefined;
  }

  private static pushCompatible(target: CloudModelWire[], model: CloudModelWire): void {
    if (!GeneralChatCloudConfigPolicy.isCompatible(model)) {
      return;
    }
    const id = GeneralChatCloudConfigPolicy.stringValue(model.id);
    const duplicate = target.some((candidate: CloudModelWire): boolean => {
      return GeneralChatCloudConfigPolicy.stringValue(candidate.id) === id;
    });
    if (!duplicate) {
      target.push(model);
    }
  }

  private static isCompatible(model: CloudModelWire): boolean {
    if (model.enabled !== true || model.auth?.type === 'subscription') {
      return false;
    }
    return GeneralChatCloudConfigPolicy.stringValue(model.base_url).trim().length > 0 &&
      GeneralChatCloudConfigPolicy.stringValue(model.model_name).trim().length > 0 &&
      GeneralChatCloudConfigPolicy.stringValue(model.api_key).trim().length > 0;
  }

  private static normalizedApiUrl(model: CloudModelWire): string {
    const baseUrl = GeneralChatCloudConfigPolicy.stringValue(model.base_url).trim().replace(/\/+$/, '');
    const provider = GeneralChatCloudConfigPolicy.stringValue(model.provider).trim().toLowerCase();
    if (provider === 'anthropic' && baseUrl.indexOf('openbitfun.com') < 0 && !baseUrl.endsWith('/v1/messages')) {
      return `${baseUrl}/v1/messages`;
    }
    return baseUrl;
  }

  private static stringValue(value?: string): string {
    return typeof value === 'string' ? value : '';
  }
}
