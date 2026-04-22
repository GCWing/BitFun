 

import { useCallback, useMemo } from 'react';
import { useTranslation, UseTranslationOptions } from 'react-i18next';
import { useI18nStore } from '../store/i18nStore';
import { i18nService } from '../core/I18nService';
import { builtinLocales, resolveLocaleId } from '../presets';
import type { LocaleId, I18nNamespace, LocaleMetadata } from '../types';

 
export interface UseI18nReturn {
   
  t: (key: string, options?: Record<string, unknown>) => string;
   
  i18n: ReturnType<typeof i18nService.getI18nInstance>;
   
  currentLanguage: LocaleId;
   
  currentLocaleMetadata: LocaleMetadata | undefined;
   
  supportedLocales: LocaleMetadata[];
   
  changeLanguage: (locale: LocaleId) => Promise<void>;
   
  isReady: boolean;
   
  isChanging: boolean;
   
  formatDate: (date: Date | number, options?: Intl.DateTimeFormatOptions) => string;
   
  formatNumber: (number: number, options?: Intl.NumberFormatOptions) => string;
   
  formatCurrency: (amount: number, currency?: string) => string;
   
  formatRelativeTime: (date: Date | number, unit?: Intl.RelativeTimeFormatUnit) => string;
   
  isRTL: boolean;
}

 
export function useI18n(
  ns?: I18nNamespace | I18nNamespace[],
  options?: UseTranslationOptions<I18nNamespace>
): UseI18nReturn {
  const { t: rawT, i18n, ready } = useTranslation(ns, options);
  
  const {
    currentLanguage,
    isInitialized,
    isChanging,
  } = useI18nStore();

  const changeLanguage = useCallback(async (locale: LocaleId) => {
    await i18nService.changeLanguage(locale);
  }, []);

  const t = useCallback(
    (key: string, translationOptions?: Record<string, unknown>) => {
      return String(rawT(key, translationOptions as any));
    },
    [rawT]
  );

  const currentLocaleMetadata = useMemo(
    () => builtinLocales.find(locale => locale.id === currentLanguage),
    [currentLanguage]
  );

  const supportedLocales = useMemo(
    () => builtinLocales,
    []
  );

  const formatDate = useCallback(
    (date: Date | number, options?: Intl.DateTimeFormatOptions) => {
      return i18nService.formatDate(date, options);
    },
    []
  );

  const formatNumber = useCallback(
    (number: number, options?: Intl.NumberFormatOptions) => {
      return i18nService.formatNumber(number, options);
    },
    []
  );

  const formatCurrency = useCallback(
    (amount: number, currency?: string) => {
      return i18nService.formatCurrency(amount, currency);
    },
    []
  );

  const formatRelativeTime = useCallback(
    (date: Date | number, unit?: Intl.RelativeTimeFormatUnit) => {
      return i18nService.formatRelativeTime(date, unit);
    },
    []
  );

  const isRTL = useMemo(
    () => currentLocaleMetadata?.rtl ?? false,
    [currentLocaleMetadata]
  );

  return {
    t,
    i18n,
    currentLanguage,
    currentLocaleMetadata,
    supportedLocales,
    changeLanguage,
    isReady: ready && isInitialized,
    isChanging,
    formatDate,
    formatNumber,
    formatCurrency,
    formatRelativeTime,
    isRTL,
  };
}

 
export function useLanguageSelector() {
  const { currentLanguage, supportedLocales, changeLanguage, isChanging } = useI18n();

  const selectLanguage = useCallback(async (locale: LocaleId) => {
    if (locale !== currentLanguage && !isChanging) {
      await changeLanguage(locale);
    }
  }, [currentLanguage, changeLanguage, isChanging]);

  return {
    currentLanguage,
    supportedLocales,
    selectLanguage,
    isChanging,
  };
}

 
export function useLanguageDetect() {
  const detectBrowserLanguage = useCallback((): LocaleId | null => {
    const browserLang = navigator.language || (navigator as any).userLanguage;

    return resolveLocaleId(browserLang);
  }, []);

  return {
    detectBrowserLanguage,
  };
}
