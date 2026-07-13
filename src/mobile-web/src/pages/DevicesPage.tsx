/**
 * Devices Page — list same-account devices and select the control target.
 * Nested sessions/chat shells were removed; the main Workspace/Session/Chat
 * surfaces talk to the selected peer via RelayHttpClient.pairedDeviceId.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { useI18n } from '../i18n';

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
function isTokenExpiredError(e: any): boolean {
  const msg = String(e?.message || '');
  return msg.includes('HTTP 401') || msg.includes('Unauthorized');
}

const DevicesPage: React.FC<Props> = ({ client, onBack }) => {
  const { t } = useI18n();
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tokenExpired, setTokenExpired] = useState(false);

  const refreshDevices = useCallback(async () => {
    if (!client.hasDelegatedIdentity) return;
    try {
      const list = await client.listDevices();
      setDevices(list);
      setTokenExpired(false);
    } catch (e: any) {
      if (isTokenExpiredError(e)) {
        setTokenExpired(true);
        setError(t('devices.tokenExpired'));
      } else {
        setError(String(e?.message || e));
      }
    }
  }, [client, t]);

  useEffect(() => {
    setLoading(true);
    refreshDevices().finally(() => setLoading(false));
    const timer = setInterval(refreshDevices, 30_000);
    return () => clearInterval(timer);
  }, [refreshDevices]);

  const selectDevice = useCallback(async (d: DeviceInfo) => {
    if (!d.online) return;
    setLoading(true);
    setError(null);
    try {
      // Probe peer host before switching the mobile control target.
      const ping = await client.sendDeviceRpc<{ resp?: string; ok?: boolean; error?: string }>(d.device_id, {
        cmd: 'host_invoke',
        command: 'peer_mode_ping',
        args: {},
      });
      if (ping.resp === 'host_invoke_result' && ping.ok === false) {
        throw new Error(ping.error || 'Peer device is not ready');
      }
      client.pairedDeviceId = d.device_id;
      onBack();
    } catch (e: any) {
      if (isTokenExpiredError(e)) {
        setTokenExpired(true);
        setError(t('devices.tokenExpired'));
      } else {
        setError(String(e?.message || e));
      }
    } finally {
      setLoading(false);
    }
  }, [client, onBack, t]);

  if (!client.hasDelegatedIdentity) {
    return (
      <div className="devices-page">
        <div className="devices-page__header">
          <button type="button" onClick={onBack}>{t('common.back')}</button>
          <h2>{t('devices.title')}</h2>
        </div>
        <div className="devices-page__empty">{t('devices.noDelegatedIdentity')}</div>
      </div>
    );
  }

  return (
    <div className="devices-page">
      <div className="devices-page__header">
        <button type="button" onClick={onBack}>{t('common.back')}</button>
        <h2>{t('devices.title')}</h2>
      </div>

      {tokenExpired && (
        <div className="devices-page__error">{t('devices.tokenExpired')}</div>
      )}
      {error && !tokenExpired && <div className="devices-page__error">{error}</div>}

      {loading && <div className="devices-page__loading">{t('devices.loading')}</div>}

      {!loading && !tokenExpired && (
        <div className="devices-page__list">
          {devices.length === 0 && <div className="devices-page__empty">{t('devices.noDevices')}</div>}
          {devices.map(d => (
            <div key={d.device_id}
              className={`devices-page__device ${d.online ? '' : 'offline'} ${client.pairedDeviceId === d.device_id ? 'selected' : ''}`}
              onClick={() => selectDevice(d)}>
              <span className="devices-page__device-name">{d.device_name}</span>
              <span className="devices-page__device-status">
                {d.online ? t('devices.online') : t('devices.offline')} {d.device_id.slice(0, 8)}
                {client.pairedDeviceId === d.device_id ? ` · ${t('devices.selected')}` : ''}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default DevicesPage;
