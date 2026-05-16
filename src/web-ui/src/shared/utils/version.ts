 

import type { VersionInfo, AboutInfo } from '../types/version';
import { i18nService } from '@/infrastructure/i18n';

 
const DEFAULT_VERSION_INFO: VersionInfo = {
  name: 'BitFun',
  version: '0.2.7',
  buildDate: new Date().toISOString(),
  buildTimestamp: Date.now(),
  isDev: import.meta.env.DEV,
  buildEnv: import.meta.env.MODE as 'development' | 'production' | 'preview'
};


let cachedVersionInfo: VersionInfo | null = null;

 
export function getVersionInfo(): VersionInfo {
  
  if (cachedVersionInfo) {
    return cachedVersionInfo;
  }

  
  if (typeof window !== 'undefined' && (window as any).__VERSION_INFO__) {
    const versionInfo = (window as any).__VERSION_INFO__ as VersionInfo;
    cachedVersionInfo = versionInfo;
    return versionInfo;
  }
  
  
  cachedVersionInfo = DEFAULT_VERSION_INFO;
  return cachedVersionInfo;
}

 
export function formatVersion(version: string, isDev: boolean): string {
  if (isDev) {
    return `${version}-dev`;
  }
  return version;
}

 
export function formatBuildDate(buildDate: string): string {
  try {
    const date = new Date(buildDate);
    return i18nService.formatDate(date, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false
    });
  } catch (_error) {
    return buildDate;
  }
}

 
export function getRelativeTime(buildTimestamp: number): string {
  const now = Date.now();
  const diff = now - buildTimestamp;
  
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  const months = Math.floor(days / 30);
  const years = Math.floor(days / 365);
  
  if (years > 0) return i18nService.t('common:time.yearsAgo', { count: years });
  if (months > 0) return i18nService.t('common:time.monthsAgo', { count: months });
  if (days > 0) return i18nService.t('common:time.daysAgo', { count: days });
  if (hours > 0) return i18nService.t('common:time.hoursAgo', { count: hours });
  if (minutes > 0) return i18nService.t('common:time.minutesAgo', { count: minutes });
  return i18nService.t('common:time.just_now');
}

 
export function getAboutInfo(): AboutInfo {
  const versionInfo = getVersionInfo();
  
  return {
    version: versionInfo,
    license: {
      type: 'MIT',
      text: 'MIT License - Copyright (c) 2025 BitFun',
      url: 'https://opensource.org/licenses/MIT'
    },
    links: {
      homepage: 'https://github.com/yourusername/bitfun',
      repository: 'https://github.com/yourusername/bitfun',
      documentation: 'https://github.com/yourusername/bitfun/wiki',
      issues: 'https://github.com/yourusername/bitfun/issues'
    }
  };
}
