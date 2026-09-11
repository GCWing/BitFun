/**
 * Devices Page — list same-account devices and pick the control target.
 *
 * The mobile stays a limited companion surface: switching only retargets
 * RelayHttpClient.targetDeviceId (device RPC data plane) and resets the
 * per-device UI state. Workspace/Session/Chat then talk to the new peer
 * through the same limited command set.
 */

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import {
  MobileBadge,
  MobileBanner,
  MobileButton,
  MobileCard,
  MobileIconButton,
  MobileListRow,
  MobilePageHeader,
  MobileStatus,
} from '@openbitfun/ui/mobile';
import {
  RelayHttpClient,
  isAccountIdentityChangedError,
} from '../services/RelayHttpClient';
import { useI18n } from '../i18n';
import { useMobileStore } from '../services/store';
import { selectAccountDevice } from '../services/accountDeviceSelection';

interface DeviceInfo {
  device_id: string;
  device_name: string;
  online: boolean;
  last_seen_at?: number | null;
}


interface Props {
  client: RelayHttpClient;
  onBack: () => void;
  onDeviceSelected?: () => void;
  accountLanding?: boolean;
  autoSelect?: boolean;
  onSignOut?: () => void;
  preferredDeviceId?: string;
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

const DevicesPage: React.FC<Props> = ({ client, onBack, onDeviceSelected = onBack, accountLanding = false, autoSelect = true, onSignOut, preferredDeviceId }) => {
  const { t, formatRelativeTime } = useI18n();
  const { connectionHealth, setControlTarget, resetForDeviceSwitch } = useMobileStore();
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [identityReady, setIdentityReady] = useState(client.hasAccountIdentity);
  const [identityChecking, setIdentityChecking] = useState(false);
  const [loading, setLoading] = useState(false);
  const [switchingId, setSwitchingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const identityRequestRef = useRef(0);
  const devicesRequestRef = useRef(0);
  const switchRequestRef = useRef(0);
  const automaticSelectionAttemptedRef = useRef(false);
  const sortedDevices = useMemo(() => {
    const listedDevices = devices.filter((device) => (
    device.device_id !== client.controllerDeviceId
  )).sort((left, right) => {
    const leftCurrent = left.device_id === client.targetDeviceId;
    const rightCurrent = right.device_id === client.targetDeviceId;
    if (leftCurrent !== rightCurrent) return leftCurrent ? -1 : 1;
    if (left.online !== right.online) return left.online ? -1 : 1;
    return (left.device_name || left.device_id).localeCompare(right.device_name || right.device_id);
    });
    return listedDevices;
  }, [client, client.controllerDeviceId, client.targetDeviceId, connectionHealth, devices]);

  const friendlyError = useCallback((value: unknown, fallbackKey: string) => {
    const message = String((value as { message?: string })?.message || value);
    if (message.includes('HTTP 401') || message.includes('Sign in with GitHub')) {
      return t(accountLanding ? 'pairing.accountSessionExpired' : 'devices.authorizationExpired');
    }
    if (message.includes('HTTP 404')) return t('devices.deviceUnavailable');
    if (message.includes('HTTP 503') || message.includes('HTTP 504')) {
      return t('devices.deviceUnavailable');
    }
    return t(fallbackKey);
  }, [accountLanding, t]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      identityRequestRef.current += 1;
      devicesRequestRef.current += 1;
      switchRequestRef.current += 1;
    };
  }, []);

  const refreshDevices = useCallback(async () => {
    if (!client.hasAccountIdentity) return;
    const requestId = ++devicesRequestRef.current;
    const isCurrent = () => (
      mountedRef.current
      && devicesRequestRef.current === requestId
    );
    try {
      const list = await client.listDevices();
      if (!isCurrent()) return;
      setDevices(list);
      setError(null);
      setIdentityReady(true);
    } catch (e: unknown) {
      if (!isCurrent()) return;
      // RelayHttpClient fences every response against its committed identity.
      // A concurrent account refresh therefore makes this request stale rather
      // than user-visible, while a successful 401 refresh + retry remains valid.
      if (isAccountIdentityChangedError(e)) return;
      const message = String((e as { message?: string })?.message || e);
      if (message.includes('Sign in with GitHub')) {
        setIdentityReady(false);
        setDevices([]);
      } else {
        setError(friendlyError(e, 'devices.loadFailed'));
      }
    }
  }, [client, friendlyError]);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setInterval> | undefined;
    const init = async () => {
      if (cancelled || !mountedRef.current) return;
      setLoading(true);
      await refreshDevices();
      if (cancelled || !mountedRef.current) return;
      setLoading(false);
      timer = setInterval(refreshDevices, 30_000);
    };
    void init();
    return () => {
      cancelled = true;
      if (timer) clearInterval(timer);
    };
  }, [refreshDevices]);

  const handleManualRefresh = useCallback(async () => {
    if (loading || switchingId) return;

    setLoading(true);
    await refreshDevices();
    if (mountedRef.current) setLoading(false);
  }, [loading, refreshDevices, switchingId]);

  const selectDevice = useCallback(async (d: DeviceInfo, probe = true) => {
    if (!d.online || switchingId) return;
    if (client.targetDeviceId === d.device_id) return;
    const requestId = ++switchRequestRef.current;
    const accountEpoch = client.accountEpoch;
    let expectedTargetEpoch = client.controlTargetEpoch;
    const isCurrent = () => (
      mountedRef.current
      && switchRequestRef.current === requestId
      && client.accountEpoch === accountEpoch
      && client.controlTargetEpoch === expectedTargetEpoch
    );
    setSwitchingId(d.device_id);
    setError(null);
    try {
      // Keep the existing probe for explicit device switches. Initial account
      // selection historically needed only the directory's online flag; do not
      // impose a new peer-mode command requirement on older desktops.
      if (probe) {
        const ping = await client.sendDeviceRpc<{ resp?: string; ok?: boolean; error?: string }>(d.device_id, {
          cmd: 'host_invoke',
          command: 'peer_mode_ping',
          args: {},
        }, { retryable: true });
        if (!isCurrent()) return;
        if (ping.resp === 'host_invoke_result' && ping.ok === false) {
          throw new Error(ping.error || t('devices.switchFailed'));
        }
      }
      client.setTargetDeviceId(d.device_id);
      expectedTargetEpoch = client.controlTargetEpoch;
      resetForDeviceSwitch();
      setControlTarget({
        deviceId: d.device_id,
        deviceName: d.device_name,
      });
      onDeviceSelected();
    } catch (e: unknown) {
      if (!isCurrent()) return;
      if (isAccountIdentityChangedError(e)) return;
      const message = String((e as { message?: string })?.message || e);
      if (message.includes('Sign in with GitHub')) {
        setIdentityReady(false);
        setDevices([]);
      } else {
        setError(friendlyError(e, 'devices.switchFailed'));
      }
    } finally {
      if (mountedRef.current && switchRequestRef.current === requestId) {
        setSwitchingId(null);
      }
    }
  }, [client, friendlyError, onDeviceSelected, resetForDeviceSwitch, setControlTarget, switchingId, t]);

  // Keep the online/scanned-device shortcut after account UI entry, without
  // making discovery failures undo authentication or retry in a render loop.
  useEffect(() => {
    if (!accountLanding || !autoSelect || !identityReady || identityChecking || loading
      || switchingId || automaticSelectionAttemptedRef.current) return;
    const target = selectAccountDevice(devices, client.controllerDeviceId, preferredDeviceId);
    if (!target) return;
    automaticSelectionAttemptedRef.current = true;
    void selectDevice(target, false);
  }, [accountLanding, autoSelect, client, devices, identityChecking, identityReady, loading, preferredDeviceId, selectDevice, switchingId]);

  const renderDeviceList = () => (
      <div className="devices-page__list">
        {sortedDevices.map((d) => {
          const isCurrent = client.targetDeviceId === d.device_id;
          const isSwitching = switchingId === d.device_id;
          const clickable = d.online && !isCurrent && !switchingId;
          return (
            <MobileListRow
              key={d.device_id}
              appearance="surface"
              className={[
                'devices-page__device',
                d.online ? 'is-online' : 'is-offline',
                isCurrent ? 'is-current' : '',
                isSwitching ? 'is-switching' : '',
              ].filter(Boolean).join(' ')}
              disabled={!clickable}
              onClick={() => clickable && selectDevice(d)}
              leading={<span className="devices-page__device-icon"><DeviceIcon /></span>}
              label={(
                <span className="devices-page__device-name-row">
                  <span className="devices-page__device-name">
                    {d.device_name || t('devices.unknownDevice')}
                  </span>
                  {isCurrent && (
                    <MobileBadge className="devices-page__badge devices-page__badge--current" tone="success">
                      {t('devices.current')}
                    </MobileBadge>
                  )}

                </span>
              )}
              supportingText={(
                <span className="devices-page__device-meta">
                  <span className={`devices-page__status-dot ${d.online ? 'is-online' : 'is-offline'}`} />
                  {d.online
                    ? t('devices.online')
                    : d.last_seen_at
                      ? t('devices.lastSeen', { time: formatRelativeTime(d.last_seen_at * 1000) })
                      : t('devices.offline')}
                </span>
              )}
              trailing={isSwitching ? (
                <span className="devices-page__device-spinner spinner" />
              ) : (
                clickable && (
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="m9 18 6-6-6-6" />
                  </svg>
                )
              )}
              selected={isCurrent}
            />
          );
        })}
      </div>
  );

  const renderBody = () => {
    if (identityChecking) {
      return (
        <>
          {sortedDevices.length > 0 && renderDeviceList()}
          <MobileStatus className="devices-page__loading" loading title={t('devices.loading')} />
        </>
      );
    }

    if (!identityReady) {
      return (
        <>
          {sortedDevices.length > 0 && renderDeviceList()}
          <MobileCard appearance="elevated" className="devices-page__empty-card">
            <MobileStatus
              action={<MobileButton className="devices-page__retry-btn" onClick={handleManualRefresh}>{t('devices.retry')}</MobileButton>}
              description={t('devices.authorizationExpired')}
              icon={<NoIdentityIcon />}
            />
          </MobileCard>
        </>
      );
    }

    if (loading && sortedDevices.length === 0) {
      return (
        <MobileStatus className="devices-page__loading" loading title={t('devices.loading')} />
      );
    }

    if (sortedDevices.length === 0) {
      // A failed directory request is not evidence that the account is empty.
      if (error) return null;
      return <MobileStatus className="devices-page__empty" description={t('devices.noDevices')} />;
    }

    return renderDeviceList();
  };

  return (
    <div className="devices-page">
      <MobilePageHeader
        className="devices-page__header"
        leading={accountLanding ? <MobileButton appearance="plain" size="sm" onClick={onBack}>{t('devices.signOut')}</MobileButton> : <MobileIconButton
          appearance="floating"
          className="devices-page__back-btn"
          icon={<BackIcon />}
          onClick={onBack}
          aria-label={t('common.back')}
        />}
        title={t('devices.title')}
        actions={<>
          {!accountLanding && onSignOut && <MobileButton appearance="plain" size="sm" onClick={onSignOut}>{t('devices.signOut')}</MobileButton>}
          <MobileIconButton
          appearance="floating"
          className="devices-page__refresh-btn"
          icon={<RefreshIcon />}
          loading={loading || identityChecking}
          onClick={handleManualRefresh}
          disabled={!!switchingId}
          aria-label={t('devices.refresh')}
          title={t('devices.refresh')}
        /></>}
      />

      {accountLanding && <p className="devices-page__description">{t('devices.accountReady')}</p>}
      {error && <MobileBanner className="devices-page__error" tone="danger">{error}</MobileBanner>}

      <div className="devices-page__body">
        {renderBody()}
      </div>
    </div>
  );
};

export default DevicesPage;
