import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Cloud,
  Link2,
  MessageCircle,
  Monitor,
  RefreshCw,
  Server,
  Smartphone,
  Undo2,
} from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import { useNotification } from '@/shared/notification-system';
import type {
  DeviceOverviewConnectionService,
  DeviceOverviewDevice,
} from '../deviceInterconnectionOverview';
import { useDeviceInterconnectionOverview } from './useDeviceInterconnectionOverview';

interface DeviceStatusControlProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onManageDevices: () => void;
}

function DeviceIcon({ device }: { device: DeviceOverviewDevice }) {
  switch (device.kind) {
    case 'mobile':
      return <Smartphone size={17} aria-hidden="true" />;
    case 'execution-host':
      return <Server size={17} aria-hidden="true" />;
    case 'message-app':
      return <MessageCircle size={17} aria-hidden="true" />;
    default:
      return <Monitor size={17} aria-hidden="true" />;
  }
}

function ConnectionServiceIcon({ service }: { service: DeviceOverviewConnectionService }) {
  return service.kind === 'self-hosted' || service.kind === 'device-service'
    ? <Server size={15} aria-hidden="true" />
    : <Cloud size={15} aria-hidden="true" />;
}

const DeviceStatusControl: React.FC<DeviceStatusControlProps> = ({
  open,
  onOpenChange,
  onManageDevices,
}) => {
  const { t } = useI18n('common');
  const { success, warning } = useNotification();
  const peerContext = usePeerDeviceModeOptional();
  const platformHint = typeof navigator === 'undefined'
    ? ''
    : `${navigator.platform ?? ''} ${navigator.userAgent ?? ''}`;
  const localDeviceLabel = /windows|win32/i.test(platformHint)
    ? t('deviceOverview.thisWindows')
    : /macintosh|macintel|mac os/i.test(platformHint)
      ? t('deviceOverview.thisMac')
      : /linux/i.test(platformHint)
        ? t('deviceOverview.thisLinux')
        : t('deviceOverview.thisDevice');
  const {
    overview,
    refresh,
    accountService,
  } = useDeviceInterconnectionOverview(localDeviceLabel);
  const [returningLocal, setReturningLocal] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const popoverLayout = useAnchoredPopoverPosition({
    open,
    anchorRef: triggerRef,
    popoverRef,
    preferredPlacement: 'top',
    alignment: 'start',
    gap: 8,
  });

  useEffect(() => {
    if (!open) return undefined;
    void refresh();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onOpenChange(false);
        triggerRef.current?.focus();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onOpenChange, open, refresh]);

  const handleReturnLocal = useCallback(async () => {
    if (!peerContext?.peerMode.active || returningLocal) return;
    setReturningLocal(true);
    try {
      const outcome = await peerContext.switchToLocal('manual');
      if (outcome === 'activated') {
        onOpenChange(false);
        success(t('deviceOverview.returnedToThisDevice'));
      }
    } catch (error) {
      warning(error instanceof Error ? error.message : String(error));
    } finally {
      setReturningLocal(false);
    }
  }, [onOpenChange, peerContext, returningLocal, success, t, warning]);

  const handleManageDevices = useCallback(() => {
    onOpenChange(false);
    onManageDevices();
  }, [onManageDevices, onOpenChange]);

  const controller = overview.devices.find(device => (
    !device.local && device.activities.includes('controlling')
  ));
  const footerSubtitle = overview.mode === 'local'
    ? t('deviceOverview.footerLocalSimple')
    : overview.peerActive
      ? t('deviceOverview.footerControlledFromHere')
      : controller
        ? t('deviceOverview.footerControlledBy', { device: controller.name })
        : t('deviceOverview.footerDistributedExecution', {
            count: overview.connectedDevices.filter(device => (
              device.activities.includes('background-execution')
            )).length,
          });
  const tooltip = `${overview.currentWorkDeviceName} · ${footerSubtitle}`;

  const deviceActivity = useCallback((device: DeviceOverviewDevice) => {
    const parts: string[] = [];
    if (device.activities.includes('current-use')) {
      parts.push(t('deviceOverview.currentUse'));
    }
    if (device.activities.includes('controlling')) {
      parts.push(t('deviceOverview.controlling'));
    }
    if (device.activities.includes('background-execution')) {
      parts.push(t('deviceOverview.executingTasks', {
        count: device.backgroundTaskCount,
      }));
    }
    return parts.join(' · ');
  }, [t]);

  const serviceContent = useMemo(() => {
    const service = overview.connectionService;
    if (!service) return null;
    switch (service.kind) {
      case 'official':
        return { label: t('deviceOverview.officialService'), detail: null };
      case 'self-hosted':
        return {
          label: t('deviceOverview.selfHostedService'),
          detail: service.host,
        };
      case 'local-network':
        return { label: t('deviceOverview.sameNetwork'), detail: null };
      case 'public-tunnel':
        return { label: t('deviceOverview.publicConnection'), detail: null };
      case 'message-app':
        return { label: t('deviceOverview.messageAppControl'), detail: service.host };
      default:
        return { label: t('deviceOverview.deviceService'), detail: service.host };
    }
  }, [overview.connectionService, t]);

  return (
    <>
      <Tooltip content={tooltip} placement="right" followCursor disabled={open}>
        <button
          ref={triggerRef}
          type="button"
          className={`bitfun-nav-panel__footer-device-status${open ? ' is-open' : ''}`}
          aria-label={tooltip}
          aria-expanded={open}
          aria-haspopup="dialog"
          onClick={() => onOpenChange(!open)}
          data-testid="nav-footer-device-status"
          data-bf-component="nav-panel"
          data-bf-part="deviceStatus"
          data-bf-state={overview.mode}
        >
          <Monitor size={16} aria-hidden="true" />
          <span className="bitfun-nav-panel__footer-device-status-copy">
            <span className="bitfun-nav-panel__footer-device-status-label">
              {overview.currentWorkDeviceName}
            </span>
            <span className="bitfun-nav-panel__footer-device-status-meta">
              {footerSubtitle}
            </span>
          </span>
        </button>
      </Tooltip>

      {open && createPortal(
        <>
          <div
            className="bitfun-nav-panel__footer-backdrop"
            onMouseDown={() => onOpenChange(false)}
            data-testid="nav-device-status-backdrop"
          />
          <div
            ref={popoverRef}
            className="bitfun-device-overview"
            role="dialog"
            aria-label={t('deviceOverview.title')}
            data-testid="nav-device-status-popover"
            data-bf-component="device-overview"
            data-bf-part="root"
            data-bf-state={overview.mode}
            data-bf-placement={popoverLayout?.placement ?? 'top'}
            style={{
              top: `${popoverLayout?.top ?? 0}px`,
              left: `${popoverLayout?.left ?? 0}px`,
              visibility: popoverLayout ? 'visible' : 'hidden',
            }}
          >
            <div className="bitfun-device-overview__header">
              <h2 className="bitfun-device-overview__title">{t('deviceOverview.title')}</h2>
            </div>
            {overview.mode === 'local' ? (
              <div
                className="bitfun-device-overview__local-device"
                data-testid="nav-device-status-summary"
              >
                <Monitor size={19} aria-hidden="true" />
                <strong>{overview.currentWorkDeviceName}</strong>
              </div>
            ) : (
              <>
                <section className="bitfun-device-overview__device-group is-primary">
                  <h3>{t('deviceOverview.currentUse')}</h3>
                  <div className="bitfun-device-overview__device-row">
                    <DeviceIcon device={overview.primaryDevice} />
                    <strong>{overview.primaryDevice.name}</strong>
                    {overview.primaryDevice.activities.includes('background-execution') && (
                      <span>{deviceActivity(overview.primaryDevice)}</span>
                    )}
                  </div>
                </section>
                <section
                  className="bitfun-device-overview__device-group"
                  data-testid="nav-device-status-connected-devices"
                >
                  <h3>{t('deviceOverview.connectedDevices')}</h3>
                  <div className="bitfun-device-overview__device-rows">
                    {overview.connectedDevices.map(device => (
                      <div
                        className="bitfun-device-overview__device-row"
                        key={device.id}
                        data-bf-device-kind={device.kind}
                        data-bf-activities={device.activities.join(' ')}
                      >
                        <DeviceIcon device={device} />
                        <strong>{device.name}</strong>
                        <span>{deviceActivity(device)}</span>
                      </div>
                    ))}
                  </div>
                </section>
              </>
            )}

            {overview.mode === 'connected' && overview.connectionService && serviceContent && (
              <div
                className="bitfun-device-overview__service"
                data-testid="nav-device-connection-service"
                data-bf-service-kind={overview.connectionService.kind}
              >
                <ConnectionServiceIcon service={overview.connectionService} />
                <span>
                  {t(overview.connectionService === accountService
                    ? 'deviceOverview.accountService'
                    : 'deviceOverview.connectionService')}
                </span>
                <strong>{serviceContent.label}</strong>
                {serviceContent.detail && <small>{serviceContent.detail}</small>}
              </div>
            )}

            {overview.topologyUnavailable && (
              <button
                type="button"
                className="bitfun-device-overview__notice"
                onClick={() => { void refresh(); }}
              >
                <RefreshCw size={14} aria-hidden="true" />
                <span>{t('deviceOverview.statusUnavailable')}</span>
              </button>
            )}

            <div className="bitfun-device-overview__actions">
              <button
                type="button"
                className="bitfun-device-overview__action"
                onClick={handleManageDevices}
                data-testid="nav-device-status-manage"
              >
                <Link2 size={16} aria-hidden="true" />
                <span>{t('deviceOverview.connectNewDevice')}</span>
              </button>
              {overview.peerActive && (
                <button
                  type="button"
                  className="bitfun-device-overview__action"
                  onClick={() => { void handleReturnLocal(); }}
                  disabled={returningLocal}
                  data-testid="nav-device-status-return-local"
                >
                  <Undo2 size={16} aria-hidden="true" />
                  <span>
                    {returningLocal
                      ? t('deviceOverview.returningToThisDevice')
                      : t('deviceOverview.backToThisDevice')}
                  </span>
                </button>
              )}
            </div>
          </div>
        </>,
        getAppearanceOverlayHost(),
      )}
    </>
  );
};

export default DeviceStatusControl;
