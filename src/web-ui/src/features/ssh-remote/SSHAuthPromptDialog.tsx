/**
 * Unified SSH authentication prompt: password, private key, or SSH agent.
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal } from '@/component-library';
import { Button } from '@/component-library';
import { Input } from '@/component-library';
import { Select } from '@/component-library';
import { IconButton } from '@/component-library';
import { FolderOpen, Key, Loader2, Lock, Server, Terminal, User } from 'lucide-react';
import type { SSHAuthMethod } from './types';
import { pickSshPrivateKeyPath } from './pickSshPrivateKeyPath';
import './SSHAuthPromptDialog.scss';

export interface SSHAuthPromptSubmitPayload {
  auth: SSHAuthMethod;
  /** When username is editable, the value from the dialog */
  username: string;
}

interface SSHAuthPromptDialogProps {
  open: boolean;
  /** Shown in the header area (e.g. user@host:port or alias) */
  targetDescription: string;
  defaultAuthMethod: 'password' | 'privateKey' | 'agent';
  defaultKeyPath?: string;
  initialUsername: string;
  /** If false, user can edit username (e.g. SSH config without User) */
  lockUsername: boolean;
  isConnecting?: boolean;
  onSubmit: (payload: SSHAuthPromptSubmitPayload) => void;
  onCancel: () => void;
}

export const SSHAuthPromptDialog: React.FC<SSHAuthPromptDialogProps> = ({
  open,
  targetDescription,
  defaultAuthMethod,
  defaultKeyPath = '~/.ssh/id_rsa',
  initialUsername,
  lockUsername,
  isConnecting = false,
  onSubmit,
  onCancel,
}) => {
  const { t } = useI18n('common');
  const [authMethod, setAuthMethod] = useState<'password' | 'privateKey' | 'agent'>(defaultAuthMethod);
  const [username, setUsername] = useState(initialUsername);
  const [password, setPassword] = useState('');
  const [keyPath, setKeyPath] = useState(defaultKeyPath);
  const [passphrase, setPassphrase] = useState('');
  const passwordRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setAuthMethod(defaultAuthMethod);
    setUsername(initialUsername);
    setPassword('');
    setKeyPath(defaultKeyPath);
    setPassphrase('');
    const focusMs = window.setTimeout(() => {
      if (defaultAuthMethod === 'password') {
        passwordRef.current?.focus();
      }
    }, 100);
    return () => window.clearTimeout(focusMs);
  }, [open, defaultAuthMethod, defaultKeyPath, initialUsername]);

  const authOptions = [
    { label: t('ssh.remote.password') || 'Password', value: 'password', icon: <Lock size={14} /> },
    { label: t('ssh.remote.privateKey') || 'Private Key', value: 'privateKey', icon: <Key size={14} /> },
    { label: t('ssh.remote.sshAgent') || 'SSH Agent', value: 'agent', icon: <Terminal size={14} /> },
  ];

  const canSubmit = (): boolean => {
    const u = username.trim();
    if (!u) return false;
    if (authMethod === 'password') return password.length > 0;
    if (authMethod === 'privateKey') return keyPath.trim().length > 0;
    return true;
  };

  const handleSubmit = () => {
    if (!canSubmit() || isConnecting) return;
    const u = username.trim();
    let auth: SSHAuthMethod;
    if (authMethod === 'password') {
      auth = { type: 'Password', password };
    } else if (authMethod === 'privateKey') {
      auth = {
        type: 'PrivateKey',
        keyPath: keyPath.trim(),
        passphrase: passphrase.trim() || undefined,
      };
    } else {
      auth = { type: 'Agent' };
    }
    onSubmit({ auth, username: u });
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && canSubmit() && !isConnecting) {
      e.preventDefault();
      handleSubmit();
    }
    if (e.key === 'Escape') {
      onCancel();
    }
  };

  const handleBrowsePrivateKey = useCallback(async () => {
    if (isConnecting) return;
    const path = await pickSshPrivateKeyPath({
      title: t('ssh.remote.pickPrivateKeyDialogTitle'),
    });
    if (path) setKeyPath(path);
  }, [isConnecting, t]);

  return (
    <Modal
      isOpen={open}
      onClose={onCancel}
      title={t('ssh.remote.authPromptTitle') || 'SSH authentication'}
      size="small"
      showCloseButton
      contentInset
    >
      <div className="ssh-auth-prompt-dialog" onKeyDown={handleKeyDown}>
        <div className="ssh-auth-prompt-dialog__description">
          <div className="ssh-auth-prompt-dialog__description-icon">
            <Server size={16} />
          </div>
          <span className="ssh-auth-prompt-dialog__description-text">{targetDescription}</span>
        </div>

        {!lockUsername && (
          <div className="ssh-auth-prompt-dialog__field">
            <Input
              label={t('ssh.remote.username')}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="root"
              prefix={<User size={16} />}
              size="medium"
              disabled={isConnecting}
            />
          </div>
        )}

        <div className="ssh-auth-prompt-dialog__field">
          <label className="ssh-auth-prompt-dialog__label">{t('ssh.remote.authMethod')}</label>
          <Select
            options={authOptions}
            value={authMethod}
            onChange={(value) => setAuthMethod(value as 'password' | 'privateKey' | 'agent')}
            size="medium"
            disabled={isConnecting}
          />
        </div>

        {authMethod === 'password' && (
          <div className="ssh-auth-prompt-dialog__field">
            <Input
              ref={passwordRef}
              label={t('ssh.remote.password')}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              prefix={<Lock size={16} />}
              size="medium"
              disabled={isConnecting}
            />
          </div>
        )}

        {authMethod === 'privateKey' && (
          <>
            <div className="ssh-auth-prompt-dialog__field">
              <Input
                label={t('ssh.remote.privateKeyPath')}
                value={keyPath}
                onChange={(e) => setKeyPath(e.target.value)}
                placeholder="~/.ssh/id_rsa"
                prefix={<Key size={16} />}
                suffix={
                  <IconButton
                    type="button"
                    variant="ghost"
                    size="small"
                    className="ssh-auth-prompt-dialog__browse-key"
                    tooltip={t('ssh.remote.browsePrivateKey')}
                    aria-label={t('ssh.remote.browsePrivateKey')}
                    disabled={isConnecting}
                    onClick={() => void handleBrowsePrivateKey()}
                  >
                    <FolderOpen size={16} />
                  </IconButton>
                }
                size="medium"
                disabled={isConnecting}
              />
            </div>
            <div className="ssh-auth-prompt-dialog__field">
              <Input
                label={t('ssh.remote.passphrase')}
                type="password"
                value={passphrase}
                onChange={(e) => setPassphrase(e.target.value)}
                placeholder={t('ssh.remote.passphraseOptional')}
                size="medium"
                disabled={isConnecting}
              />
            </div>
          </>
        )}

        {authMethod === 'agent' && (
          <p className="ssh-auth-prompt-dialog__hint">{t('ssh.remote.authPromptAgentHint')}</p>
        )}

        <div className="ssh-auth-prompt-dialog__actions">
          <Button variant="secondary" onClick={onCancel} disabled={isConnecting}>
            {t('actions.cancel')}
          </Button>
          <Button
            variant="primary"
            onClick={handleSubmit}
            disabled={!canSubmit() || isConnecting}
          >
            {isConnecting ? (
              <>
                <Loader2 size={14} className="ssh-auth-prompt-dialog__spinner" />
                {t('ssh.remote.connecting')}
              </>
            ) : (
              t('ssh.remote.connect')
            )}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default SSHAuthPromptDialog;
