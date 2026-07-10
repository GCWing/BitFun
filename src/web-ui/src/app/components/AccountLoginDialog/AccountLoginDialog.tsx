/**
 * Account login dialog with three fields: username, password, auth server.
 * Visual style mirrors the SSH new-connection dialog.
 *
 * Login flow:
 * 1. User enters credentials and clicks Login
 * 2. accountLogin() → relay authenticates, returns has_cloud_settings flag
 * 3. If first login (has_cloud_settings=false): auto-sync local config+sessions to cloud
 * 4. If non-first login (has_cloud_settings=true): show confirmation dialog —
 *    cloud config will overwrite local. User must agree or cancel (logout).
 * 5. After sync: connect WS device routing, close dialog.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { Modal, Button, Input, Alert } from '@/component-library';
import { User, Lock, Server, LogIn, Monitor, CloudDownload, RefreshCw } from 'lucide-react';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type { OnlineDeviceInfo } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AccountLoginDialog.scss';

const log = createLogger('AccountLoginDialog');

interface AccountLoginDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export const AccountLoginDialog: React.FC<AccountLoginDialogProps> = ({
  isOpen,
  onClose,
}) => {
  const { t } = useI18n('common');
  const { success } = useNotification();
  const { workspacePath } = useCurrentWorkspace();

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [authServer, setAuthServer] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [onlineDevices, setOnlineDevices] = useState<OnlineDeviceInfo[]>([]);
  const [refreshingDevices, setRefreshingDevices] = useState(false);
  // Cloud overwrite confirmation state
  const [pendingOverwrite, setPendingOverwrite] = useState<{
    server: string;
    username: string;
    password: string;
  } | null>(null);

  useEffect(() => {
    if (!isOpen) {
      setUsername('');
      setPassword('');
      setAuthServer('');
      setError(null);
      setLoading(false);
      setPendingOverwrite(null);
    } else {
      // Pre-fill username + auth server from persisted credential hint
      remoteConnectAPI.accountGetCredentialHint().then((hint) => {
        if (hint) {
          setUsername(hint.username);
          setAuthServer(hint.relay_url);
        }
      });
    }
  }, [isOpen]);

  const validate = useCallback(() => {
    if (!username.trim() || !password.trim() || !authServer.trim()) {
      setError(t('accountLogin.emptyFields'));
      return false;
    }
    setError(null);
    return true;
  }, [username, password, authServer, t]);

  const doAutoSync = useCallback(
    async (isFirstLogin: boolean) => {
      // Export local config JSON for upload (first login) or skip (non-first)
      let configJson = '{}';
      if (isFirstLogin) {
        try {
          const exported = await configAPI.exportConfig();
          configJson = JSON.stringify(exported);
        } catch (e) {
          log.warn('Failed to export local config, using empty', e);
        }
      }

      const wp = workspacePath || '/';
      const result = await remoteConnectAPI.accountAutoSync(isFirstLogin, wp, configJson);
      log.info(
        `Auto-sync done: settings=${result.settings_synced} exported=${result.sessions_exported} imported=${result.sessions_imported}`,
      );
    },
    [workspacePath],
  );

  const handleLogin = useCallback(async () => {
    if (!validate()) return;
    setLoading(true);
    setError(null);
    const server = authServer.trim();
    const user = username.trim();
    const pass = password;
    try {
      const result = await remoteConnectAPI.accountLogin(server, user, pass);

      if (result.has_cloud_settings) {
        // Non-first login: pause and ask user to confirm cloud overwrite
        setPendingOverwrite({ server, username: user, password: pass });
        setLoading(false);
        return;
      }

      // First login: auto-sync (upload local config + sessions to cloud)
      await doAutoSync(true);

      // Connect WS device routing
      remoteConnectAPI.accountConnectDevices().catch((err) => {
        log.warn('accountConnectDevices failed after login', err);
      });

      success(t('accountLogin.loginSuccess', { user_id: result.user_id }));
      onClose();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [validate, authServer, username, password, success, t, onClose, doAutoSync]);

  const handleConfirmOverwrite = useCallback(async () => {
    if (!pendingOverwrite) return;
    setLoading(true);
    setError(null);
    try {
      // Non-first login: download cloud config, overwrite local
      await doAutoSync(false);

      remoteConnectAPI.accountConnectDevices().catch((err) => {
        log.warn('accountConnectDevices failed after login', err);
      });

      success(t('accountLogin.loginSuccess', { user_id: pendingOverwrite.username }));
      setPendingOverwrite(null);
      onClose();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [pendingOverwrite, doAutoSync, success, t, onClose]);

  const handleCancelOverwrite = useCallback(async () => {
    // User declined cloud overwrite → logout and close
    try {
      await remoteConnectAPI.accountLogout();
    } catch (e) {
      log.warn('Logout after cancel failed', e);
    }
    setPendingOverwrite(null);
    onClose();
  }, [onClose]);

  const refreshDevices = useCallback(async () => {
    setRefreshingDevices(true);
    try {
      const devices = await remoteConnectAPI.accountOnlineDevices();
      setOnlineDevices(devices);
    } catch (e) {
      log.warn('refreshDevices failed', e);
    } finally {
      setRefreshingDevices(false);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      refreshDevices();
      const interval = setInterval(refreshDevices, 10000);
      return () => clearInterval(interval);
    }
  }, [isOpen, refreshDevices]);

  // ── Cloud overwrite confirmation view ──────────────────────────────────
  if (pendingOverwrite) {
    return (
      <Modal
        isOpen={isOpen}
        onClose={handleCancelOverwrite}
        title={t('shared:features.accountLogin')}
        size="medium"
        showCloseButton
        closeOnOverlayClick={false}
        contentClassName="modal__content--fill-flex"
      >
        <div className="account-login-dialog">
          <div className="account-login-dialog__scroll">
            <div className="account-login-dialog__overwrite-notice">
              <CloudDownload size={32} />
              <p>{t('accountLogin.cloudOverwriteWarning')}</p>
              <p className="account-login-dialog__overwrite-detail">
                {t('accountLogin.cloudOverwriteDetail')}
              </p>
            </div>
          </div>
          <div className="account-login-dialog__actions">
            <Button
              variant="secondary"
              size="small"
              onClick={handleCancelOverwrite}
              disabled={loading}
            >
              {t('accountLogin.disagree')}
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={handleConfirmOverwrite}
              disabled={loading}
            >
              {loading ? t('accountLogin.processing') : t('accountLogin.agree')}
            </Button>
          </div>
        </div>
      </Modal>
    );
  }

  // ── Login form view ─────────────────────────────────────────────────────
  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('shared:features.accountLogin')}
      size="medium"
      showCloseButton
      closeOnOverlayClick={false}
      contentClassName="modal__content--fill-flex"
    >
      <div className="account-login-dialog">
        {error && (
          <div className="account-login-dialog__error-banner">
            <Alert
              type="error"
              message={error}
              closable
              onClose={() => setError(null)}
              className="account-login-dialog__error-alert"
            />
          </div>
        )}

        <div className="account-login-dialog__scroll">
          <div className="account-login-dialog__form">
            <div className="account-login-dialog__field">
              <Input
                label={t('accountLogin.username')}
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder=""
                prefix={<User size={16} />}
                size="medium"
                disabled={loading}
              />
            </div>
            <div className="account-login-dialog__field">
              <Input
                label={t('accountLogin.password')}
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder=""
                prefix={<Lock size={16} />}
                size="medium"
                disabled={loading}
              />
            </div>
            <div className="account-login-dialog__field">
              <Input
                label={t('accountLogin.authServer')}
                type="url"
                value={authServer}
                onChange={(e) => setAuthServer(e.target.value)}
                placeholder={t('accountLogin.authServerPlaceholder')}
                prefix={<Server size={16} />}
                size="medium"
                disabled={loading}
              />
            </div>
          </div>

          {onlineDevices.length > 0 && (
            <div className="account-login-dialog__devices">
              <div className="account-login-dialog__devices-header">
                <Monitor size={14} />
                <span>{t('accountLogin.devices')}</span>
                <Button
                  variant="ghost"
                  size="small"
                  onClick={refreshDevices}
                  disabled={refreshingDevices}
                >
                  <RefreshCw size={12} className={refreshingDevices ? 'spinning' : ''} />
                </Button>
              </div>
              <div className="account-login-dialog__device-list">
                {onlineDevices.map((d) => (
                  <div key={d.device_id} className="account-login-dialog__device-item">
                    <span className="account-login-dialog__device-name">{d.device_name}</span>
                    <span className="account-login-dialog__device-id">{d.device_id.slice(0, 8)}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        <div className="account-login-dialog__actions">
          <Button
            variant="secondary"
            size="small"
            onClick={onClose}
            disabled={loading}
          >
            {t('accountLogin.cancel')}
          </Button>
          <Button
            variant="primary"
            size="small"
            onClick={handleLogin}
            disabled={loading}
          >
            <LogIn size={14} />
            {loading ? t('accountLogin.processing') : t('accountLogin.login')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default AccountLoginDialog;
