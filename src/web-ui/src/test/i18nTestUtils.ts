import commonMessages from '@/locales/en-US/common.json';
import componentsMessages from '@/locales/en-US/components.json';
import flowChatMessages from '@/locales/en-US/flow-chat.json';

type TranslationOptions = Record<string, unknown> & {
  defaultValue?: string;
};

type TranslationKey = string | readonly string[];

const resources: Record<string, unknown> = {
  common: commonMessages,
  components: componentsMessages,
  'flow-chat': flowChatMessages,
};

export function createTestI18nT(defaultNamespace = 'common') {
  return (key: TranslationKey, options?: TranslationOptions): string => {
    const keys = Array.isArray(key) ? key : [key];
    for (const candidate of keys) {
      const value = resolveMessage(candidate, defaultNamespace);
      if (typeof value === 'string') {
        return interpolate(value, options);
      }
    }

    if (typeof options?.defaultValue === 'string') {
      return interpolate(options.defaultValue, options);
    }

    return String(keys[0] ?? '');
  };
}

function resolveMessage(key: string, defaultNamespace: string): unknown {
  const separatorIndex = key.indexOf(':');
  const namespace = separatorIndex > 0 ? key.slice(0, separatorIndex) : defaultNamespace;
  const path = separatorIndex > 0 ? key.slice(separatorIndex + 1) : key;
  return getPath(resources[namespace], path);
}

function getPath(source: unknown, path: string): unknown {
  return path.split('.').reduce<unknown>((current, segment) => {
    if (current && typeof current === 'object' && segment in current) {
      return (current as Record<string, unknown>)[segment];
    }
    return undefined;
  }, source);
}

function interpolate(message: string, options?: TranslationOptions): string {
  return message.replace(/\{\{(\w+)\}\}/g, (match, token: string) => {
    const value = options?.[token];
    return value == null ? match : String(value);
  });
}
