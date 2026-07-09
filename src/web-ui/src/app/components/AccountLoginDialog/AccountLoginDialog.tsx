/**
 * Account login dialog with three fields: username, password, auth server.
 * Visual style mirrors the SSH new-connection dialog.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal, Button, Input, Alert } from '@/component-library';
import { User, Lock, Server, LogIn } from 'lucide-react';
import './AccountLoginDialog.scss';

interface AccountLoginDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export const AccountLoginDialog: React.FC<AccountLoginDialogProps> = ({
  isOpen,
  onClose,
}) => {
  const { t } = useI18n('common');

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [authServer, setAuthServer] = useState('');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) {
      setUsername('');
      setPassword('');
      setAuthServer('');
      setError(null);
    }
  }, [isOpen]);

  const handleLogin = useCallback(() => {
    if (!username.trim() || !password.trim() || !authServer.trim()) {
      setError(t('accountLogin.emptyFields'));
      return;
    }
    setError(null);
    onClose();
  }, [username, password, authServer, t, onClose]);

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
              />
            </div>
          </div>
        </div>

        <div className="account-login-dialog__actions">
          <Button
            variant="secondary"
            size="small"
            onClick={onClose}
          >
            {t('accountLogin.cancel')}
          </Button>
          <Button
            variant="primary"
            size="small"
            onClick={handleLogin}
          >
            <LogIn size={14} />
            {t('accountLogin.login')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default AccountLoginDialog;
