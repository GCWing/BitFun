import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Card, CardBody, CardFooter, CardHeader, Icon, ScrollArea } from '@bitfun/ui';
import { createPortal } from 'react-dom';
import { Cloud, Monitor, Server, Smartphone, Undo2 } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import { useNotification } from '@/shared/notification-system';
import {
  ChatAppBrandIcon,
  type ChatAppBrand,
} from '../../RemoteConnectDialog/ChatAppBrandIcon';
import {
  selectActivityFacts,
  selectAttachedGroups,
  type DeviceOverviewActivityFact,
  type DeviceOverviewConnectionService,
  type DeviceOverviewDevice,
  type DeviceOverviewDeviceKind,
} from '../deviceInterconnectionOverview';
import { useDeviceInterconnectionOverview } from './useDeviceInterconnectionOverview';
import { DeviceArtwork } from './DeviceArtwork';

interface DeviceStatusControlProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onManageDevices: () => void;
}

/** Resolve the provider from backend ids, aliases, or display names. */
function chatAppBrandFromIdentity(identity: string | null | undefined): ChatAppBrand | null {
  const normalized = identity?.trim().toLocaleLowerCase();
  if (!normalized) return null;
  if (normalized.includes('telegram')) return 'telegram';
  if (normalized.includes('feishu') || normalized.includes('lark')) return 'feishu';
  if (normalized.includes('weixin') || normalized.includes('wechat')) return 'weixin';
  return null;
}

function DeviceIcon({
  identity,
  kind,
  size = 17,
}: {
  identity?: string | null;
  kind: DeviceOverviewDeviceKind;
  size?: number;
}) {
  const catalogSize = size <= 11 ? '2xs' : size <= 13 ? 'xs' : size <= 15 ? 'sm' : size <= 17 ? 'md' : 'lg';
  switch (kind) {
    case 'mobile':
      return <Smartphone size={size} aria-hidden="true" />;
    case 'execution-host':
      return <Server size={size} aria-hidden="true" />;
    case 'message-app': {
      const chatApp = chatAppBrandFromIdentity(identity);
      if (chatApp) return <ChatAppBrandIcon app={chatApp} size={size} />;
      return <Icon name="side-chat" size={catalogSize} aria-hidden="true" />;
    }
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
  const attachedMessageAppIdentity = useMemo(() => (
    overview.connectedDevices.find(device => device.kind === 'message-app')?.name
  ), [overview.connectedDevices]);
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

  const deviceDisplayName = useCallback((device: DeviceOverviewDevice) => {
    const chatApp = device.kind === 'message-app'
      ? chatAppBrandFromIdentity(`${device.id} ${device.name}`)
      : null;
    if (chatApp === 'telegram') return 'Telegram';
    if (chatApp === 'feishu') return t('remoteConnect.feishu');
    if (chatApp === 'weixin') return t('remoteConnect.weixin');
    return device.name;
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
                data-bf-device-kind={group.kind}
                key={group.kind}
              >
                <DeviceIcon
                  identity={group.kind === 'message-app' ? attachedMessageAppIdentity : null}
                  kind={group.kind}
                  size={13}
                />
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
          <Card
            ref={popoverRef}
            appearance="raised"
            className="bitfun-device-overview"
            gap="none"
            padding="none"
            radius="lg"
            role="dialog"
            aria-label={t('deviceOverview.title')}
            data-testid="nav-device-status-popover"
            data-bf-product-component="device-overview"
            data-bf-product-part="root"
            data-bf-state={overview.mode}
            data-bf-placement={popoverLayout?.placement ?? 'top'}
            style={{
              top: `${popoverLayout?.top ?? 0}px`,
              left: `${popoverLayout?.left ?? 0}px`,
              visibility: popoverLayout ? 'visible' : 'hidden',
            }}
          >
            <CardHeader
              className="bitfun-device-overview__header"
              contentAlign="center"
              title={<h2 className="bitfun-device-overview__title">{t('deviceOverview.title')}</h2>}
            />
            <ScrollArea className="bitfun-device-overview__scroll">
            <CardBody className="bitfun-device-overview__body">
              <div className="bitfun-device-overview__summary" data-testid="nav-device-status-summary">
                <DeviceArtwork device={overview.primaryDevice} />
                <span className="bitfun-device-overview__device-name" title={overview.currentWorkDeviceName}>
                  {overview.currentWorkDeviceName}
                </span>
                {overview.mode === 'connected' && (
                  <span className="bitfun-device-overview__activity">
                    {deviceActivity(overview.primaryDevice)}
                  </span>
                )}
              </div>
              {overview.mode === 'connected' && (
                <>
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
                          <span className="bitfun-device-overview__device-icon" aria-hidden="true">
                            <DeviceIcon
                              identity={`${device.id} ${device.name}`}
                              kind={device.kind}
                              size={16}
                            />
                          </span>
                          <strong>{deviceDisplayName(device)}</strong>
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
                  leadingIcon={<Icon name="refresh" size="lg" />}
                  className="bitfun-device-overview__notice"
                  onClick={() => { void refresh(); }}
                >
                  {t('deviceOverview.statusUnavailable')}
                </Button>
              )}
            </CardBody>
            </ScrollArea>

            <CardFooter align="center" className="bitfun-device-overview__actions">
              <Button
                className="bitfun-device-overview__action"
                variant="primary"
                size="sm"
                leadingIcon={<Icon name="link" size="sm" />}
                onClick={handleManageDevices}
                data-testid="nav-device-status-manage"
              >
                {t('accountLogin.connectDevices')}
              </Button>
              {overview.peerActive && (
                <Button
                  className="bitfun-device-overview__action"
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
            </CardFooter>
          </Card>
        </>,
        getAppearanceOverlayHost(),
      )}
    </>
  );
};

export default DeviceStatusControl;
