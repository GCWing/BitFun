import React, { createContext, useCallback, useContext, useMemo, useState } from 'react';
import {
  disabledPrivacyStatus,
  privacyAPI,
  type AcceptPrivacyRequest,
  type PrivacyEffectiveMode,
  type PrivacyStatus,
} from '@/infrastructure/api/service-api/PrivacyAPI';

interface PrivacyContextValue {
  status: PrivacyStatus | null;
  initialize: () => Promise<PrivacyStatus>;
  refresh: (locale: string) => Promise<PrivacyStatus>;
  accept: (request: AcceptPrivacyRequest) => Promise<PrivacyStatus>;
  enterNotAccepted: (locale: string) => Promise<PrivacyStatus>;
  markViewed: (policyUpdatedAt: string, locale: string) => Promise<PrivacyStatus>;
  applyCollectionPolicy: (
    mode: PrivacyEffectiveMode,
    locale: string,
  ) => Promise<PrivacyStatus>;
}

const unavailable = async (): Promise<PrivacyStatus> => disabledPrivacyStatus;

const PrivacyContext = createContext<PrivacyContextValue>({
  status: null,
  initialize: unavailable,
  refresh: unavailable,
  accept: unavailable,
  enterNotAccepted: unavailable,
  markViewed: unavailable,
  applyCollectionPolicy: unavailable,
});

export const PrivacyProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [status, setStatus] = useState<PrivacyStatus | null>(null);

  const update = useCallback(async (operation: () => Promise<PrivacyStatus>) => {
    const next = await operation();
    setStatus(next);
    return next;
  }, []);
  const initialize = useCallback(() => update(() => privacyAPI.initialize()), [update]);
  const refresh = useCallback(
    (locale: string) => update(() => privacyAPI.getStatus(locale)),
    [update],
  );
  const accept = useCallback(
    (request: AcceptPrivacyRequest) => update(() => privacyAPI.accept(request)),
    [update],
  );
  const enterNotAccepted = useCallback(
    (locale: string) => update(() => privacyAPI.enterNotAccepted(locale)),
    [update],
  );
  const markViewed = useCallback(
    (policyUpdatedAt: string, locale: string) =>
      update(() => privacyAPI.markViewed(policyUpdatedAt, locale)),
    [update],
  );
  const applyCollectionPolicy = useCallback(
    (mode: PrivacyEffectiveMode, locale: string) =>
      update(() => privacyAPI.applyCollectionPolicy(mode, locale)),
    [update],
  );

  const value = useMemo(
    () => ({
      status,
      initialize,
      refresh,
      accept,
      enterNotAccepted,
      markViewed,
      applyCollectionPolicy,
    }),
    [accept, applyCollectionPolicy, enterNotAccepted, initialize, markViewed, refresh, status],
  );
  return <PrivacyContext.Provider value={value}>{children}</PrivacyContext.Provider>;
};

export const usePrivacy = (): PrivacyContextValue => useContext(PrivacyContext);
