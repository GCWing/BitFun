 

import type { LocaleId, LocaleMetadata } from '../types';

 
export const DEFAULT_LOCALE: LocaleId = 'zh-CN';

 
export const DEFAULT_FALLBACK_LOCALE: LocaleId = 'en-US';

 
export const builtinLocales: LocaleMetadata[] = [
  {
    id: 'zh-CN',
    name: '简体中文',
    englishName: 'Simplified Chinese',
    nativeName: '简体中文',
    rtl: false,
    dateFormat: 'YYYY年MM月DD日',
    numberFormat: {
      decimal: '.',
      thousands: ',',
    },
    builtin: true,
  },
  {
    id: 'en-US',
    name: 'English',
    englishName: 'English (US)',
    nativeName: 'English',
    rtl: false,
    dateFormat: 'MM/DD/YYYY',
    numberFormat: {
      decimal: '.',
      thousands: ',',
    },
    builtin: true,
  },
];

 
export function getLocaleMetadata(localeId: LocaleId): LocaleMetadata | undefined {
  return builtinLocales.find(locale => locale.id === localeId);
}

 
export function isLocaleSupported(localeId: string): localeId is LocaleId {
  return builtinLocales.some(locale => locale.id === localeId);
}

 
export function getSupportedLocaleIds(): LocaleId[] {
  return builtinLocales.map(locale => locale.id);
}

 
export const DEFAULT_NAMESPACE = 'common';

 
export const ALL_NAMESPACES = [
  'common',
  'flow-chat',
  'tools',
  'settings',
  'errors',
  'notifications',
  'components',
  'scenes/apps',
  'scenes/subagents',
  'scenes/profile',
  'scenes/skills',
  'scenes/tools',
  'scenes/capabilities',
  'panels/files',
  'panels/git',
  'panels/terminal',
  'settings/ai-model',
  'settings/agentic-tools',
  'settings/mcp',
  'settings/basics',
  'settings/ai-features',
  'settings/session-config',
  'settings/debug',
  'settings/editor',
  'settings/skills',
  'settings/agents',
  'settings/default-model',
] as const;
