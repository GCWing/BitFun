/**
 * Devices Page — list same-account devices and pick the control target.
 *
 * The mobile stays a limited companion surface: switching only retargets
 * RelayHttpClient.pairedDeviceId (device RPC data plane) and resets the
 * per-device UI state. Workspace/Session/Chat then talk to the new peer
 * through the same limited command set.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { useI18n } from '../i18n';
import { useMobileStore } from '../services/store';

interface DeviceInfo {
  device_id: string;
  device_name: string;
  online: boolean;
}

interface Props {
  client: RelayHttpClient;
  onBack: () => void;
}

/** Check whether an error is an HTTP 401 (token expired/unauthorized). */
function isTokenExpiredError(e: unknown): boolean {
  const msg = String((e as { message?: string })?.message || '');
  return msg.includes('HTTP 401') || msg.includes('Unauthorized');
}

const BackIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="m15 18-6-6 6-6" />
  </svg>
);

const RefreshIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
    <path d="M21 3v5h-5" />
    <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
    <path d="M3 21v-5h5" />
  </svg>
);

const DeviceIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
    <line x1="8" y1="21" x2="16" y2="21" />
    <line x1="12" y1="17" x2="12" y2="21" />
  </svg>
);

const NoIdentityIcon = () => (
  <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="8" r="4" />
    <path d="M6 21v-1a6 6 0 0 1 9-5.2" />
    <circle cx="18" cy="18" r="4" />
    <path d="M18 16.5v1.8l1.2 1.2" />
  </svg>
);

const DevicesPage: React.FC<Props> = ({ client, onBack }) => {
  const { t } = useI18n();
  const { setControlTarget, resetForDeviceSwitch } = useMobileStore();
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [identityReady, setIdentityReady] = useState(client.hasDelegatedIdentity);
  const [identityChecking, setIdentityChecking] = useState(!client.hasDelegatedIdentity);
  const [loading, setLoading] = useState(false);
  const [switchingId, setSwitchingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tokenExpired, setTokenExpired] = useState(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refreshDevices = useCallback(async () => {
    if (!client.hasDelegatedIdentity) return;
    try {
      const list = await client.listDevices();
      if (!mountedRef.current) return;
      setDevices(list);
      setTokenExpired(false);
      setError(null);
    } catch (e: unknown) {
      if (!mountedRef.current) return;
      if (isTokenExpiredError(e)) {
        setTokenExpired(true);
      } else {
        setError(String((e as { message?: string })?.message || e));
      }
    }
  }, [client]);

  // Acquire the delegated identity lazily: the desktop may have logged into
  // its account after this mobile session was paired.
  const ensureIdentity = useCallback(async () => {
    if (client.hasDelegatedIdentity) {
      setIdentityReady(true);
      setIdentityChecking(false);
      return true;
    }
    setIdentityChecking(true);
    let granted = false;
    try {
      granted = await client.requestDelegatedIdentity();
    } catch {
      granted = false;
    }
    if (mountedRef.current) {
      setIdentityReady(granted);
      setIdentityChecking(false);
    }
    return granted;
  }, [client]);

  useEffect(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    const init = async () => {
      const granted = await ensureIdentity();
      if (!granted || !mountedRef.current) return;
      setLoading(true);
      await refreshDevices();
      if (mountedRef.current) setLoading(false);
      timer = setInterval(refreshDevices, 30_000);
    };
    void init();
    return () => {
      if (timer) clearInterval(timer);
    };
  }, [ensureIdentity, refreshDevices]);

  const handleManualRefresh = useCallback(async () => {
    if (loading || switchingId) return;
    const granted = await ensureIdentity();
    if (!granted) return;
    setLoading(true);
    await refreshDevices();
    if (mountedRef.current) setLoading(false);
  }, [ensureIdentity, loading, refreshDevices, switchingId]);

  const selectDevice = useCallback(async (d: DeviceInfo) => {
    if (!d.online || switchingId) return;
    if (client.pairedDeviceId === d.device_id) return;
    setSwitchingId(d.device_id);
    setError(null);
    try {
      // Probe the peer host before switching the mobile control target.
      const ping = await client.sendDeviceRpc<{ resp?: string; ok?: boolean; error?: string }>(d.device_id, {
        cmd: 'host_invoke',
        command: 'peer_mode_ping',
        args: {},
      });
      if (ping.resp === 'host_invoke_result' && ping.ok === false) {
        throw new Error(ping.error || t('devices.switchFailed'));
      }
      client.pairedDeviceId = d.device_id;
      resetForDeviceSwitch();
      setControlTarget({
        deviceId: d.device_id,
        deviceName: d.device_name,
        isHome: d.device_id === client.homeDeviceId,
      });
      onBack();
    } catch (e: unknown) {
      if (!mountedRef.current) return;
      if (isTokenExpiredError(e)) {
        setTokenExpired(true);
      } else {
        setError(String((e as { message?: string })?.message || e) || t('devices.switchFailed'));
      }
    } finally {
      if (mountedRef.current) setSwitchingId(null);
    }
  }, [client, onBack, resetForDeviceSwitch, setControlTarget, switchingId, t]);

  const renderBody = () => {
    if (identityChecking) {
      return (
        <div className="devices-page__loading">
          <span className="spinner" />
          {t('devices.loading')}
        </div>
      );
    }

    if (!identityReady) {
      return (
        <div className="devices-page__empty-card">
          <span className="devices-page__empty-icon"><NoIdentityIcon /></span>
          <p className="devices-page__empty-text">{t('devices.noDelegatedIdentity')}</p>
          <button type="button" className="devices-page__retry-btn" onClick={handleManualRefresh}>
            {t('devices.retry')}
          </button>
        </div>
      );
    }

    if (tokenExpired) {
      return (
        <div className="devices-page__empty-card">
          <span className="devices-page__empty-icon"><NoIdentityIcon /></span>
          <p className="devices-page__empty-text">{t('devices.tokenExpired')}</p>
        </div>
      );
    }

    if (loading && devices.length === 0) {
      return (
        <div className="devices-page__loading">
          <span className="spinner" />
          {t('devices.loading')}
        </div>
      );
    }

    if (devices.length === 0) {
      return <div className="devices-page__empty">{t('devices.noDevices')}</div>;
    }

    return (
      <div className="devices-page__list">
        {devices.map((d) => {
          const isCurrent = client.pairedDeviceId === d.device_id;
          const isHome = client.homeDeviceId === d.device_id;
          const isSwitching = switchingId === d.device_id;
          const clickable = d.online && !isCurrent && !switchingId;
          return (
            <button
              key={d.device_id}
              type="button"
              className={[
                'devices-page__device',
                d.online ? 'is-online' : 'is-offline',
                isCurrent ? 'is-current' : '',
                isSwitching ? 'is-switching' : '',
              ].filter(Boolean).join(' ')}
              disabled={!clickable}
              onClick={() => clickable && selectDevice(d)}
            >
              <span className="devices-page__device-icon"><DeviceIcon /></span>
              <span className="devices-page__device-copy">
                <span className="devices-page__device-name-row">
                  <span className="devices-page__device-name">{d.device_name}</span>
                  {isCurrent && (
                    <span className="devices-page__badge devices-page__badge--current">
                      {t('devices.current')}
                    </span>
                  )}
                  {isHome && !isCurrent && (
                    <span className="devices-page__badge">
                      {t('devices.pairedDesktop')}
                    </span>
                  )}
                </span>
                <span className="devices-page__device-meta">
                  <span className={`devices-page__status-dot ${d.online ? 'is-online' : 'is-offline'}`} />
                  {d.online ? t('devices.online') : t('devices.offline')}
                  <span className="devices-page__device-id">{d.device_id.slice(0, 8)}</span>
                </span>
              </span>
              {isSwitching ? (
                <span className="devices-page__device-spinner spinner" />
              ) : (
                clickable && (
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="m9 18 6-6-6-6" />
                  </svg>
                )
              )}
            </button>
          );
        })}
      </div>
    );
  };

  return (
    <div className="devices-page">
      <div className="devices-page__header">
        <button type="button" className="devices-page__back-btn" onClick={onBack} aria-label={t('common.back')}>
          <BackIcon />
        </button>
        <h2 className="devices-page__title">{t('devices.title')}</h2>
        <button
          type="button"
          className="devices-page__refresh-btn"
          onClick={handleManualRefresh}
          disabled={loading || !!switchingId}
          aria-label={t('devices.refresh')}
          title={t('devices.refresh')}
        >
          <RefreshIcon />
        </button>
      </div>

      {error && !tokenExpired && <div className="devices-page__error">{error}</div>}

      <div className="devices-page__body">
        {renderBody()}
      </div>
    </div>
  );
};

export default DevicesPage;
