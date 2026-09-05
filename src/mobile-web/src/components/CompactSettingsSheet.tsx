import React from 'react';
import {
  MobileBadge,
  MobileButton,
  MobileCard,
  MobileIconButton,
  MobileListRow,
  MobileSheet,
} from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import LanguageToggleButton from './LanguageToggleButton';

interface SettingsDevice {
  device_id: string;
  device_name: string;
  online: boolean;
}

interface CompactSettingsSheetProps {
  accountLabel: string | null;
  devices: SettingsDevice[];
  isDark: boolean;
  onClose: () => void;
  onDisconnectRequest: () => void;
  onSelectDevice: (device: SettingsDevice) => void;
  onToggleTheme: () => void;
  open: boolean;
  renderDeviceIcon: (name: string) => React.ReactNode;
  selectedDeviceId: string | null;
}

function ThemeToggleIcon({ isDark }: { isDark: boolean }) {
  return isDark ? (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" /></svg>
  ) : (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.93 4.93l1.42 1.42M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.42-1.41M17.66 6.34l1.41-1.41" /></svg>
  );
}

export default function CompactSettingsSheet({
  accountLabel,
  devices,
  isDark,
  onClose,
  onDisconnectRequest,
  onSelectDevice,
  onToggleTheme,
  open,
  renderDeviceIcon,
  selectedDeviceId,
}: CompactSettingsSheetProps) {
  const { t } = useI18n();

  return (
    <MobileSheet
      className="harmony-sidebar__settings-sheet"
      headerAction={<MobileIconButton appearance="plain" size="sm" onClick={onClose} aria-label={t('common.close')} icon={<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg>} />}
      onOpenChange={onClose}
      open={open}
      title={t('shared.features.settings')}
    >
      <div className="harmony-sidebar__settings-scroll">
        <h3>{t('settings.accountSection')}</h3>
        <MobileCard className="harmony-sidebar__account-card">
          <span className="harmony-sidebar__account-avatar" aria-hidden="true">
            {accountLabel
              ? accountLabel.slice(0, 1).toLocaleUpperCase()
              : <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="13" rx="2" /><path d="M8 21h8M12 17v4" /></svg>}
          </span>
          <span className="harmony-sidebar__account-copy">
            <strong>{accountLabel ? t('settings.currentAccount') : t('settings.notSignedIn')}</strong>
            <small>{accountLabel || t('settings.connectedByQr')}</small>
          </span>
          {accountLabel && <MobileBadge className="harmony-sidebar__verified" tone="success">{t('settings.signedIn')}</MobileBadge>}
        </MobileCard>

        <h3>{t('settings.generalSection')}</h3>
        <MobileCard padding="none" className="harmony-sidebar__settings-card">
          <MobileButton appearance="plain" block className="harmony-sidebar__settings-row" onClick={onToggleTheme}>
            <span className="harmony-sidebar__settings-row-icon"><ThemeToggleIcon isDark={isDark} /></span>
            <span className="harmony-sidebar__settings-label">{t('settings.appearance')}</span>
            <small>{isDark ? 'Dark' : 'Light'}</small>
            <span className="harmony-sidebar__settings-chevron" aria-hidden="true">›</span>
          </MobileButton>
          <div className="harmony-sidebar__settings-row">
            <span className="harmony-sidebar__settings-row-icon" aria-hidden="true"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9" /><path d="M3.5 9h17M3.5 15h17M12 3c2.2 2.45 3.3 5.45 3.3 9S14.2 18.55 12 21M12 3C9.8 5.45 8.7 8.45 8.7 12s1.1 6.55 3.3 9" /></svg></span>
            <span className="harmony-sidebar__settings-label">{t('settings.language')}</span>
            <LanguageToggleButton className="harmony-sidebar__settings-language" />
            <span className="harmony-sidebar__settings-chevron" aria-hidden="true">›</span>
          </div>
        </MobileCard>

        <h3>{t('settings.modelSection')}</h3>
        <MobileCard padding="none" className="harmony-sidebar__settings-card">
          <div className="harmony-sidebar__settings-row">
            <span className="harmony-sidebar__settings-row-icon" aria-hidden="true"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65"><rect x="3" y="3" width="7" height="7" rx="2" /><rect x="14" y="3" width="7" height="7" rx="2" /><rect x="3" y="14" width="7" height="7" rx="2" /><rect x="14" y="14" width="7" height="7" rx="2" /></svg></span>
            <span className="harmony-sidebar__settings-label">{t('settings.defaultModel')}</span>
            <small>{t('settings.followDesktop')}</small>
          </div>
        </MobileCard>

        <h3>{t('settings.devicesSection')}</h3>
        <MobileCard padding="none" className="harmony-sidebar__settings-card harmony-sidebar__settings-card--devices">
          {devices.map((device) => {
            const current = device.device_id === selectedDeviceId;
            return (
              <MobileListRow
                appearance="plain"
                className={`harmony-sidebar__settings-device${current ? ' is-current' : ''}`}
                disabled={!device.online}
                key={device.device_id}
                label={device.device_name || device.device_id}
                leading={<span className="harmony-sidebar__settings-device-icon">{renderDeviceIcon(device.device_name || device.device_id)}</span>}
                onClick={() => onSelectDevice(device)}
                selected={current}
                supportingText={current ? t('settings.currentDevice') : device.online ? t('devices.online') : t('devices.offline')}
                trailing={<span className={`harmony-sidebar__status${device.online ? ' is-online' : ''}`} />}
              />
            );
          })}
        </MobileCard>

        <h3>{t('settings.aboutSection')}</h3>
        <MobileCard padding="none" className="harmony-sidebar__settings-card">
          <div className="harmony-sidebar__settings-row harmony-sidebar__settings-row--static"><span>{t('shared.product.remote')}</span><small>{t('settings.platform')}</small></div>
        </MobileCard>

        <MobileButton appearance="danger" block className="harmony-sidebar__settings-disconnect" onClick={onDisconnectRequest}>
          {t('sessions.disconnect')}
        </MobileButton>
      </div>
    </MobileSheet>
  );
}
