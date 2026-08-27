import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '@bitfun/ui';
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
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import { useNotification } from '@/shared/notification-system';
import {
  selectActivityFacts,
  selectAttachedGroups,
  type DeviceOverviewActivityFact,
  type DeviceOverviewConnectionService,
  type DeviceOverviewDevice,
  type DeviceOverviewDeviceKind,
} from '../deviceInterconnectionOverview';
import { useDeviceInterconnectionOverview } from './useDeviceInterconnectionOverview';

interface DeviceStatusControlProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onManageDevices: () => void;
}

function DeviceIcon({ kind, size = 17 }: { kind: DeviceOverviewDeviceKind; size?: number }) {
  switch (kind) {
    case 'mobile':
      return <Smartphone size={size} aria-hidden="true" />;
    case 'execution-host':
      return <Server size={size} aria-hidden="true" />;
    case 'message-app':
      return <MessageCircle size={size} aria-hidden="true" />;
    default:
      return <Monitor size={size} aria-hidden="true" />;
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

  const activityFactSentence = useCallback((fact: DeviceOverviewActivityFact) => {
    switch (fact.kind) {
      case 'local':
        return t('deviceOverview.footerLocalSimple');
      case 'controlled-from-here':
        return t('deviceOverview.footerControlledFromHere');
      case 'controlled-by':
        return t('deviceOverview.footerControlledBy', { device: fact.device });
      case 'controllers':
        return t('deviceOverview.footerControllers', { count: fact.count });
      default:
        return t('deviceOverview.footerDistributedExecution', { count: fact.count });
    }
  }, [t]);
  const activityLines = useMemo(
    () => selectActivityFacts(overview).map(activityFactSentence),
    [activityFactSentence, overview],
  );
  const attachedGroups = useMemo(() => selectAttachedGroups(overview), [overview]);
  const accessibleSummary = [overview.currentWorkDeviceName, ...activityLines].join(' · ');

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
      default:
        return { label: t('deviceOverview.deviceService'), detail: service.host };
    }
  }, [overview.connectionService, t]);

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className={`bitfun-nav-panel__footer-device-status${open ? ' is-open' : ''}`}
        aria-label={accessibleSummary}
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => onOpenChange(!open)}
        data-testid="nav-footer-device-status"
        data-bf-component="nav-panel"
        data-bf-part="deviceStatus"
        data-bf-state={overview.mode}
      >
        <DeviceIcon kind={overview.primaryDevice.kind} size={15} />
        <span className="bitfun-nav-panel__footer-device-status-label">
          {overview.currentWorkDeviceName}
        </span>
        {attachedGroups.length > 0 && (
          <span
            className="bitfun-nav-panel__footer-device-status-attached"
            aria-hidden="true"
          >
            {attachedGroups.map(group => (
              <span
                className="bitfun-nav-panel__footer-device-status-attached-group"
                key={group.kind}
              >
                <DeviceIcon kind={group.kind} size={13} />
                {group.count > 1 && (
                  <span className="bitfun-nav-panel__footer-device-status-attached-count">
                    {group.count}
                  </span>
                )}
              </span>
            ))}
          </span>
        )}
      </button>

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
                <DeviceIcon kind={overview.primaryDevice.kind} size={19} />
                <strong>{overview.currentWorkDeviceName}</strong>
              </div>
            ) : (
              <>
                <section className="bitfun-device-overview__device-group is-primary">
                  <h3>{t('deviceOverview.currentUse')}</h3>
                  <div className="bitfun-device-overview__device-row">
                    <DeviceIcon kind={overview.primaryDevice.kind} />
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
                        <DeviceIcon kind={device.kind} />
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
              <Button
                variant="outline"
                size="sm"
                leadingIcon={<RefreshCw />}
                className="bitfun-device-overview__notice"
                onClick={() => { void refresh(); }}
              >
                {t('deviceOverview.statusUnavailable')}
              </Button>
            )}

            <div className="bitfun-device-overview__actions">
              <Button
                variant="fill"
                size="sm"
                leadingIcon={<Link2 />}
                onClick={handleManageDevices}
                data-testid="nav-device-status-manage"
              >
                {t('deviceOverview.connectNewDevice')}
              </Button>
              {overview.peerActive && (
                <Button
                  variant="outline"
                  size="sm"
                  leadingIcon={<Undo2 />}
                  onClick={() => { void handleReturnLocal(); }}
                  disabled={returningLocal}
                  data-testid="nav-device-status-return-local"
                >
                  {returningLocal
                    ? t('deviceOverview.returningToThisDevice')
                    : t('deviceOverview.backToThisDevice')}
                </Button>
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
