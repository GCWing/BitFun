/**
 * Unified SSH authentication prompt.
 */

import {
  Button,
  Field,
  Icon,
  IconButton,
  Input,
  Select,
  Tooltip,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@openbitfun/ui';
import React, { useState, useEffect, useRef, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';

import { FolderOpen, Key, Loader2, Lock, Server } from 'lucide-react';
import type { SSHAuthMethod } from './types';
import {
  pickSshCertificatePath,
  pickSshPrivateKeyPath,
} from './pickSshPrivateKeyPath';
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
  defaultAuthMethod: 'password' | 'privateKey' | 'agent' | 'keyboardInteractive';
  defaultKeyPath?: string;
  defaultCertificatePath?: string;
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
  defaultCertificatePath = '',
  initialUsername,
  lockUsername,
  isConnecting = false,
  onSubmit,
  onCancel,
}) => {
  const { t } = useI18n('common');
  const [authMethod, setAuthMethod] = useState<
    'password' | 'privateKey' | 'agent' | 'keyboardInteractive'
  >(defaultAuthMethod);
  const [username, setUsername] = useState(initialUsername);
  const [password, setPassword] = useState('');
  const [keyPath, setKeyPath] = useState(defaultKeyPath);
  const [passphrase, setPassphrase] = useState('');
  const [certificatePath, setCertificatePath] = useState(defaultCertificatePath);
  const [verificationCode, setVerificationCode] = useState('');
  const passwordRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setAuthMethod(defaultAuthMethod);
    setUsername(initialUsername);
    setPassword('');
    setKeyPath(defaultKeyPath);
    setPassphrase('');
    setCertificatePath(defaultCertificatePath);
    setVerificationCode('');
    const focusMs = window.setTimeout(() => {
      if (defaultAuthMethod === 'password' || defaultAuthMethod === 'keyboardInteractive') {
        passwordRef.current?.focus();
      }
    }, 100);
    return () => window.clearTimeout(focusMs);
  }, [open, defaultAuthMethod, defaultKeyPath, defaultCertificatePath, initialUsername]);

  const authOptions = [
    { label: t('ssh.remote.password') || 'Password', value: 'password', icon: <Lock size={14} /> },
    { label: t('ssh.remote.privateKey') || 'Private Key', value: 'privateKey', icon: <Key size={14} /> },
    { label: t('ssh.remote.sshAgent'), value: 'agent', icon: <Key size={14} /> },
    {
      label: t('ssh.remote.keyboardInteractive'),
      value: 'keyboardInteractive',
      icon: <Lock size={14} />,
    },
  ];

  const canSubmit = (): boolean => {
    const u = username.trim();
    if (!u) return false;
    if (authMethod === 'password') return password.length > 0;
    if (authMethod === 'privateKey') return keyPath.trim().length > 0;
    if (authMethod === 'keyboardInteractive') {
      return password.length > 0 || verificationCode.length > 0;
    }
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
        certificatePath: certificatePath.trim() || undefined,
      };
    } else if (authMethod === 'agent') {
      auth = { type: 'Agent' };
    } else {
      auth = {
        type: 'KeyboardInteractive',
        responses: [password, verificationCode].filter(Boolean),
      };
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

  const handleBrowseCertificate = useCallback(async () => {
    if (isConnecting) return;
    const path = await pickSshCertificatePath({
      title: t('ssh.remote.pickCertificateDialogTitle'),
    });
    if (path) setCertificatePath(path);
  }, [isConnecting, t]);

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => { if (!nextOpen) onCancel(); }}
      size="sm"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{t('ssh.remote.authPromptTitle') || 'SSH authentication'}</DialogTitle>
        </DialogHeading>
        <DialogClose />
      </DialogHeader>
      <DialogBody>
      <div className="ssh-auth-prompt-dialog" onKeyDown={handleKeyDown} data-openbitfun-component="ssh-remote" data-openbitfun-part="authDialog">
        <div className="ssh-auth-prompt-dialog__description" data-openbitfun-component="ssh-remote" data-openbitfun-part="authDescription">
          <div className="ssh-auth-prompt-dialog__description-icon">
            <Server size={16} />
          </div>
          <span className="ssh-auth-prompt-dialog__description-text">{targetDescription}</span>
        </div>

        {!lockUsername && (
          <div className="ssh-auth-prompt-dialog__field" data-openbitfun-component="ssh-remote" data-openbitfun-part="authField">
            <Field label={t('ssh.remote.username')} controlWidth="fill">
              <Input
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="root"
                leading={<Icon name="user" size="md" />}
                disabled={isConnecting}
              />
            </Field>
          </div>
        )}

        <div className="ssh-auth-prompt-dialog__field" data-openbitfun-component="ssh-remote" data-openbitfun-part="authField">
          <label className="ssh-auth-prompt-dialog__label">{t('ssh.remote.authMethod')}</label>
          <Select
            options={authOptions}
            value={authMethod}
            onValueChange={(value) => setAuthMethod(
              value as 'password' | 'privateKey' | 'agent' | 'keyboardInteractive',
            )}
            size="md"
            disabled={isConnecting}
          />
        </div>

        {authMethod === 'password' && (
          <div className="ssh-auth-prompt-dialog__field">
            <Field label={t('ssh.remote.password')} controlWidth="fill">
              <Input
                ref={passwordRef}
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                leading={<Lock size={16} />}
                disabled={isConnecting}
              />
            </Field>
          </div>
        )}

        {authMethod === 'privateKey' && (
          <>
            <div className="ssh-auth-prompt-dialog__field">
              <Field label={t('ssh.remote.privateKeyPath')} controlWidth="fill">
                <Input
                  value={keyPath}
                  onChange={(e) => setKeyPath(e.target.value)}
                  placeholder="~/.ssh/id_rsa"
                  leading={<Key size={16} />}
                  trailing={
                    <Tooltip content={t('ssh.remote.browsePrivateKey')}>
                      <IconButton
                        type="button"
                        size="sm"
                        className="ssh-auth-prompt-dialog__browse-key"
                        aria-label={t('ssh.remote.browsePrivateKey')}
                        disabled={isConnecting}
                        onClick={() => void handleBrowsePrivateKey()}
                        icon={<FolderOpen size={16} />}
                      />
                    </Tooltip>
                  }
                  disabled={isConnecting}
                />
              </Field>
            </div>
            <div className="ssh-auth-prompt-dialog__field">
              <Field label={t('ssh.remote.passphrase')} controlWidth="fill">
                <Input
                  type="password"
                  value={passphrase}
                  onChange={(e) => setPassphrase(e.target.value)}
                  placeholder={t('ssh.remote.passphraseOptional')}
                  disabled={isConnecting}
                />
              </Field>
            </div>
            <div className="ssh-auth-prompt-dialog__field">
              <Field label={t('ssh.remote.certificatePath')} controlWidth="fill">
                <Input
                  value={certificatePath}
                  onChange={(e) => setCertificatePath(e.target.value)}
                  placeholder={t('ssh.remote.certificatePathOptional')}
                  trailing={
                    <Tooltip content={t('ssh.remote.browseCertificate')}>
                      <IconButton
                        type="button"
                        size="sm"
                        className="ssh-auth-prompt-dialog__browse-key"
                        aria-label={t('ssh.remote.browseCertificate')}
                        disabled={isConnecting}
                        onClick={() => void handleBrowseCertificate()}
                        icon={<FolderOpen size={16} />}
                      />
                    </Tooltip>
                  }
                  disabled={isConnecting}
                />
              </Field>
            </div>
          </>
        )}

        {authMethod === 'keyboardInteractive' && (
          <>
            <div className="ssh-auth-prompt-dialog__field">
              <Field label={t('ssh.remote.challengePassword')} controlWidth="fill">
                <Input
                  ref={passwordRef}
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  leading={<Lock size={16} />}
                  disabled={isConnecting}
                />
              </Field>
            </div>
            <div className="ssh-auth-prompt-dialog__field">
              <Field label={t('ssh.remote.verificationCode')} controlWidth="fill">
                <Input
                  type="password"
                  value={verificationCode}
                  onChange={(e) => setVerificationCode(e.target.value)}
                  placeholder={t('ssh.remote.optional')}
                  disabled={isConnecting}
                />
              </Field>
            </div>
          </>
        )}

        <div className="ssh-auth-prompt-dialog__actions" data-openbitfun-component="ssh-remote" data-openbitfun-part="authActions">
          <Button variant="outline" size="sm" onClick={onCancel} disabled={isConnecting}>
            {t('actions.cancel')}
          </Button>
          <Button
            variant="fill"
            size="sm"
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
          </DialogBody>
    </Dialog>
  );
};

export default SSHAuthPromptDialog;
