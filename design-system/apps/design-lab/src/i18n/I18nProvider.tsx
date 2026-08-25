import {
  createContext,
  useCallback,
  useLayoutEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  resolveDesignLabLocale,
  resolveFirstDesignLabLocale,
  translateFromCatalog,
  type DesignLabLocale,
  type TranslateParams,
} from "./core.mjs";
import { messages, type MessageKey } from "./messages";

const STORAGE_KEY = "bitfun.design-lab.locale";

export interface I18nContextValue {
  locale: DesignLabLocale;
  setLocale: (locale: DesignLabLocale) => void;
  t: (key: MessageKey, params?: TranslateParams) => string;
}

export const I18nContext = createContext<I18nContextValue | null>(null);

function detectUrlLocale(): DesignLabLocale | null {
  try {
    return resolveDesignLabLocale(new URLSearchParams(window.location.search).get("lang"));
  } catch {
    return null;
  }
}

function detectInitialLocale(): DesignLabLocale {
  try {
    const storedLocale = resolveDesignLabLocale(window.localStorage.getItem(STORAGE_KEY));
    if (storedLocale) {
      return storedLocale;
    }
  } catch {
    // Storage can be unavailable in hardened or private browser contexts.
  }

  const urlLocale = detectUrlLocale();
  if (urlLocale) {
    return urlLocale;
  }

  const browserLocales = typeof navigator === "undefined"
    ? []
    : navigator.languages?.length
      ? navigator.languages
      : [navigator.language];
  return resolveFirstDesignLabLocale(browserLocales, "en-US");
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<DesignLabLocale>(detectInitialLocale);

  useLayoutEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = "ltr";
    try {
      window.localStorage.setItem(STORAGE_KEY, locale);
    } catch {
      // Keep the active locale even when persistence is unavailable.
    }
  }, [locale]);

  const setLocale = useCallback((nextLocale: DesignLabLocale) => {
    setLocaleState(nextLocale);
  }, []);

  const value = useMemo<I18nContextValue>(() => ({
    locale,
    setLocale,
    t: (key, params) => translateFromCatalog(messages, locale, key, params),
  }), [locale, setLocale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}
