/**
 * Device surface switcher — the account's devices, one click apart.
 *
 * Mounted as a full-width row above the NavPanel footer whenever an account is
 * signed in. Picking a device only changes which one this window renders:
 * every attached device keeps executing its own sessions, so work can be
 * running on several machines at once.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ActionItem, Icon, Menu, MenuItem, MenuSection, MenuSeparator } from '@bitfun/ui';
import { createPortal } from 'react-dom';
import { Monitor, MonitorSmartphone, Loader2, Unplug } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useNotification } from '@/shared/notification-system';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { usePeerDeviceModeOptional } from './peerDeviceContextState';
import { useAccountDeviceRoster, type DeviceRosterEntry } from './useAccountDeviceRoster';
import { isDeviceBusy, subscribeDeviceActivity } from './deviceActivity';
import './DeviceSurfaceSwitcher.scss';

/** Re-render on any activity change; the roster is small enough to recompute. */
function useDeviceActivityVersion(): number {
  const [version, setVersion] = useState(0);
  useEffect(() => subscribeDeviceActivity(() => setVersion(v => v + 1)), []);
  return version;
}

export const DeviceSurfaceSwitcher: React.FC = () => {
  const { t } = useI18n('common');
  const { success, warning } = useNotification();
  const peerDevice = usePeerDeviceModeOptional();
  const { loggedIn, localDeviceId, devices } = useAccountDeviceRoster();
  const activityVersion = useDeviceActivityVersion();

  const [open, setOpen] = useState(false);
  const [switching, setSwitching] = useState(false);
  const latestSwitchRequestRef = useRef(0);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const layout = useAnchoredPopoverPosition({
    open,
    anchorRef: triggerRef,
    popoverRef,
    preferredPlacement: 'top',
    alignment: 'start',
    gap: 6,
  });

  const peerMode = peerDevice?.peerMode;
  const attachments = peerDevice?.attachments;
  const activeDeviceId = peerMode?.active ? peerMode.deviceId : localDeviceId;

  const attachedIds = useMemo(
    () => new Set((attachments ?? []).map(attachment => attachment.deviceId)),
    [attachments],
  );

  /** Activity key: the rendered local device reports under `null`. */
  const activityKeyFor = useCallback(
    (device: DeviceRosterEntry) => (device.isLocal ? null : device.deviceId),
    [],
  );

  const busyElsewhereCount = useMemo(() => {
    void activityVersion;
    return devices.filter(
      device => device.deviceId !== activeDeviceId && isDeviceBusy(activityKeyFor(device)),
    ).length;
  }, [devices, activeDeviceId, activityKeyFor, activityVersion]);

  const currentDevice = useMemo(
    () => devices.find(device => device.deviceId === activeDeviceId) ?? null,
    [devices, activeDeviceId],
  );

  const currentLabel = peerMode?.active
    ? peerMode.deviceName
    : currentDevice?.deviceName ?? t('accountLogin.thisDevice');

  const handleSelect = useCallback(async (device: DeviceRosterEntry) => {
    if (!peerDevice) {
      return;
    }
    setOpen(false);
    // While another target is still activating, selecting the rendered device
    // is meaningful: it supersedes that activation (A -> B -> A).
    if (device.deviceId === activeDeviceId && !switching) {
      return;
    }
    const requestId = ++latestSwitchRequestRef.current;
    setSwitching(true);
    try {
      let outcome: 'activated' | 'superseded';
      if (device.isLocal) {
        outcome = await peerDevice.switchToLocal();
        if (outcome === 'activated' && latestSwitchRequestRef.current === requestId) {
          success(t('accountLogin.deviceSwitcher.switchedLocal'));
        }
      } else {
        outcome = await peerDevice.switchToDevice(device.deviceId, device.deviceName);
        if (outcome === 'activated' && latestSwitchRequestRef.current === requestId) {
          success(t('accountLogin.deviceSwitcher.switched', { name: device.deviceName }));
        }
      }
    } catch (error) {
      if (latestSwitchRequestRef.current === requestId) {
        warning(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (latestSwitchRequestRef.current === requestId) {
        setSwitching(false);
      }
    }
  }, [peerDevice, switching, activeDeviceId, success, warning, t]);

  const handleDisconnect = useCallback(async (
    event: React.MouseEvent,
    device: DeviceRosterEntry,
  ) => {
    event.stopPropagation();
    if (!peerDevice || switching) {
      return;
    }
    setSwitching(true);
    try {
      await peerDevice.disconnectDevice(device.deviceId);
      success(t('accountLogin.deviceSwitcher.disconnected', { name: device.deviceName }));
    } catch (error) {
      warning(error instanceof Error ? error.message : String(error));
    } finally {
      setSwitching(false);
    }
  }, [peerDevice, switching, success, warning, t]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [open]);

  if (!loggedIn || !peerDevice) {
    return null;
  }

  const isRemote = peerMode?.active === true;

  return (
    <>
      <ActionItem
        ref={triggerRef}
        className={[
          'bitfun-device-switcher',
          isRemote && 'is-remote',
        ].filter(Boolean).join(' ')}
        data-testid="device-surface-switcher"
        data-bf-component="peer-device"
        data-bf-part="switcher"
        data-bf-state={isRemote ? 'remote' : 'local'}
        aria-expanded={open}
        aria-label={t('accountLogin.deviceSwitcher.open')}
        title={t('accountLogin.deviceSwitcher.open')}
        leading={switching ? (
          <Loader2 className="bitfun-device-switcher__icon is-spinning" aria-hidden="true" />
        ) : (
          <Monitor className="bitfun-device-switcher__icon" aria-hidden="true" />
        )}
        metadata={(
          <>
            {busyElsewhereCount > 0 && (
              <span
                className="bitfun-device-switcher__elsewhere"
                data-bf-component="peer-device"
                data-bf-part="switcherElsewhere"
                title={t('accountLogin.deviceSwitcher.othersRunning', { count: busyElsewhereCount })}
              >
                <MonitorSmartphone size={11} aria-hidden="true" />
                {busyElsewhereCount}
              </span>
            )}
            <Icon name="chevron-up" size="lg" className="bitfun-device-switcher__chevron" aria-hidden="true" style={{ width: 13, height: 13 }} />
          </>
        )}
        onClick={() => setOpen(value => !value)}
      >
        <span
          className="bitfun-device-switcher__label"
          data-bf-component="peer-device"
          data-bf-part="switcherLabel"
        >
          {currentLabel}
        </span>
      </ActionItem>

      {open && createPortal(
        <>
          <div
            className="bitfun-device-switcher__backdrop"
            onClick={() => setOpen(false)}
          />
          <Menu
            ref={popoverRef}
            className="bitfun-device-switcher__menu"
            data-testid="device-surface-switcher-menu"
            data-bf-component="peer-device"
            data-bf-part="switcherMenu"
            style={{
              top: `${layout?.top ?? 0}px`,
              left: `${layout?.left ?? 0}px`,
              visibility: layout ? 'visible' : 'hidden',
            }}
          >
            <MenuSection title={t('accountLogin.deviceSwitcher.title')}>
              {devices.map(device => {
                const isCurrent = device.deviceId === activeDeviceId;
                const busy = isDeviceBusy(activityKeyFor(device));
                const attached = attachedIds.has(device.deviceId);
                const selectable = device.online && (!isCurrent || switching);
                return (
                  <MenuItem
                    key={device.deviceId}
                    role="menuitemradio"
                    checked={isCurrent}
                    aria-disabled={!selectable}
                    data-bf-component="peer-device"
                    data-bf-part="switcherItem"
                    leading={(
                      <span
                        className={[
                          'bitfun-device-switcher__dot',
                          busy && 'is-busy',
                          !device.online && 'is-offline',
                        ].filter(Boolean).join(' ')}
                        data-bf-component="peer-device"
                        data-bf-part="switcherStatusDot"
                        data-bf-state={[
                          busy && 'busy',
                          !device.online && 'offline',
                        ].filter(Boolean).join(' ') || undefined}
                        aria-hidden="true"
                      />
                    )}
                    metadata={(
                      <span className="bitfun-device-switcher__item-metadata">
                        {device.isLocal && (
                          <span className="bitfun-device-switcher__tag">
                            {t('accountLogin.thisDevice')}
                          </span>
                        )}
                        {busy && (
                          <span className="bitfun-device-switcher__tag is-busy">
                            {t('accountLogin.deviceSwitcher.running')}
                          </span>
                        )}
                        {!device.online && (
                          <span className="bitfun-device-switcher__tag">
                            {t('accountLogin.offline')}
                          </span>
                        )}
                        {isCurrent && <Icon name="check-line" size="xs" aria-hidden="true" />}
                      </span>
                    )}
                    actions={attached ? [{
                      id: 'disconnect',
                      label: t('accountLogin.deviceSwitcher.disconnectHint'),
                      icon: <Unplug size={13} aria-hidden="true" />,
                      disabled: switching,
                      onClick: event => { void handleDisconnect(event, device); },
                    }] : undefined}
                    onClick={() => {
                      if (selectable) void handleSelect(device);
                    }}
                  >
                    {device.deviceName}
                  </MenuItem>
                );
              })}
              {devices.length <= 1 && (
                <div className="bitfun-device-switcher__empty">
                  {t('accountLogin.deviceSwitcher.noDevices')}
                </div>
              )}
            </MenuSection>
            <MenuSeparator />
            <div className="bitfun-device-switcher__hint">
              {t('accountLogin.deviceSwitcher.hint')}
            </div>
          </Menu>
        </>,
        getAppearanceOverlayHost(),
      )}
    </>
  );
};

export default DeviceSurfaceSwitcher;
