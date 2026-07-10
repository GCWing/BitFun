/**
 * Account login dialog with three fields: username, password, auth server.
 * Visual style mirrors the SSH new-connection dialog.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal, Button, Input, Alert } from '@/component-library';
import { User, Lock, Server, LogIn, RefreshCw, Monitor } from 'lucide-react';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type { OnlineDeviceInfo } from '@/infrastructure/api/service-api/RemoteConnectAPI';
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

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [authServer, setAuthServer] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [onlineDevices, setOnlineDevices] = useState<OnlineDeviceInfo[]>([]);
  const [refreshingDevices, setRefreshingDevices] = useState(false);

  useEffect(() => {
    if (!isOpen) {
      setUsername('');
      setPassword('');
      setAuthServer('');
      setError(null);
      setLoading(false);
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

  const handleLogin = useCallback(async () => {
    if (!validate()) return;
    setLoading(true);
    setError(null);
    try {
      const result = await remoteConnectAPI.accountLogin(authServer.trim(), username.trim(), password);
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
  }, [validate, authServer, username, password, success, t, onClose]);

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

  const handleSyncSessions = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      await remoteConnectAPI.accountExportAllSessions('/');
      const imported = await remoteConnectAPI.accountImportRemoteSessions('/');
      success(t('accountLogin.syncSuccess'));
      log.info(`Imported ${imported.length} remote sessions`);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [success, t]);

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
            variant="secondary"
            size="small"
            onClick={handleSyncSessions}
            disabled={loading}
          >
            <RefreshCw size={14} />
            {t('accountLogin.syncSessions')}
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
