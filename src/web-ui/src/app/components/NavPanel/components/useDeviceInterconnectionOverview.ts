import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useAccountLoginState } from '@/infrastructure/account/useAccountLoginState';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import {
  remoteConnectAPI,
  type DeviceInfo,
  type RemoteConnectStatus,
} from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import { useDispatchJobStore } from '@/features/dispatch/dispatchJobStore';
import {
  connectionServiceFromRelayUrl,
  projectDeviceInterconnectionOverview,
  type DeviceOverviewConnectionService,
  type DeviceOverviewDispatchJob,
} from '../deviceInterconnectionOverview';

const TOPOLOGY_POLL_MS = 15_000;

export function useDeviceInterconnectionOverview(fallbackLocalDeviceName: string) {
  const account = useAccountLoginState();
  const peerContext = usePeerDeviceModeOptional();
  const dispatchJobs = useDispatchJobStore(state => state.jobs);

  const [localDevice, setLocalDevice] = useState<DeviceInfo | null>(null);
  const [remoteStatus, setRemoteStatus] = useState<RemoteConnectStatus | null>(null);
  const [remoteStatusState, setRemoteStatusState] = useState<'loading' | 'ready' | 'unavailable'>('loading');
  const [accountService, setAccountService] = useState<DeviceOverviewConnectionService | null>(null);
  const refreshGenerationRef = useRef(0);

  const refresh = useCallback(async () => {
    const generation = ++refreshGenerationRef.current;
    const isCurrent = () => refreshGenerationRef.current === generation;

    const devicePromise = remoteConnectAPI.getDeviceInfo()
      .then(device => {
        if (isCurrent()) setLocalDevice(device);
      })
      .catch(() => undefined);

    const statusPromise = remoteConnectAPI.getStatus()
      .then(status => {
        if (!isCurrent()) return;
        setRemoteStatus(status);
        setRemoteStatusState('ready');
      })
      .catch(() => {
        if (!isCurrent()) return;
        // A failed probe cannot prove a previous connection still exists.
        setRemoteStatus(null);
        setRemoteStatusState('unavailable');
      });

    const relayPromise = account.loggedIn
      ? remoteConnectAPI.accountGetCredentialHint().then(hint => {
          if (isCurrent()) {
            setAccountService(connectionServiceFromRelayUrl(hint?.relay_url));
          }
        })
      : Promise.resolve().then(() => {
          if (isCurrent()) setAccountService(null);
        });

    await Promise.all([devicePromise, statusPromise, relayPromise]);
  }, [account.loggedIn]);

  useEffect(() => {
    void refresh();
    const poll = window.setInterval(() => {
      if (document.visibilityState === 'visible') void refresh();
    }, TOPOLOGY_POLL_MS);
    const refreshWhenVisible = () => {
      if (document.visibilityState === 'visible') void refresh();
    };
    document.addEventListener('visibilitychange', refreshWhenVisible);
    return () => {
      refreshGenerationRef.current += 1;
      window.clearInterval(poll);
      document.removeEventListener('visibilitychange', refreshWhenVisible);
    };
  }, [refresh]);

  useEffect(() => {
    const unlistenLogin = api.listen('account://login-state', () => {
      void refresh();
    });
    const unlistenPresence = api.listen('account://device-presence', () => {
      void refresh();
    });
    return () => {
      unlistenLogin();
      unlistenPresence();
    };
  }, [refresh]);

  const projectedDispatchJobs = useMemo<DeviceOverviewDispatchJob[]>(() => (
    Object.values(dispatchJobs).map(job => {
      if (job.target.kind === 'local') {
        return {
          id: job.jobId,
          state: job.state,
          target: { kind: 'local' as const },
        };
      }
      if (job.target.kind === 'ssh') {
        return {
          id: job.jobId,
          state: job.state,
          target: {
            kind: 'ssh' as const,
            id: job.target.connectionId,
            name: job.target.displayName,
          },
        };
      }
      return {
        id: job.jobId,
        state: job.state,
        target: {
          kind: 'device' as const,
          id: job.target.deviceId,
          name: job.target.displayName,
        },
      };
    })
  ), [dispatchJobs]);

  const peer = useMemo(() => (
    peerContext?.peerMode.active
      ? {
          deviceId: peerContext.peerMode.deviceId,
          deviceName: peerContext.peerMode.deviceName,
        }
      : null
  ), [peerContext?.peerMode]);

  const localDeviceName = localDevice?.device_name?.trim() || fallbackLocalDeviceName;
  const overview = useMemo(() => projectDeviceInterconnectionOverview({
    localDeviceName,
    peer,
    remoteStatus,
    remoteStatusState,
    dispatchJobs: projectedDispatchJobs,
    accountService,
  }), [
    accountService,
    localDeviceName,
    peer,
    projectedDispatchJobs,
    remoteStatus,
    remoteStatusState,
  ]);

  return {
    overview,
    refresh,
    remoteStatus,
    accountService,
  };
}
