/**
 * Remote Port Forwarding Dialog
 *
 * Manual mappings only: nothing is forwarded until the user asks for it. The
 * table deliberately separates the remote port (the thing the user thinks in)
 * from the local address (an allocation that can move when the port is taken).
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Modal, Button, Input, Checkbox, Empty } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createLogger } from '@/shared/utils/logger';
import { AlertTriangle, Copy, ExternalLink, Plus, RefreshCw, Radar, X } from 'lucide-react';
import { sshApi } from './sshApi';
import type { PortForward, RemoteListeningPort } from './types';
import './PortForwardDialog.scss';

const log = createLogger('PortForwardDialog');

/**
 * How often the table refreshes its counters while the dialog is open.
 *
 * The backend keeps the numbers in atomics and snapshots on demand, so polling
 * costs one cheap command; anything faster just makes the digits flicker.
 */
const REFRESH_INTERVAL_MS = 2000;

interface PortForwardDialogProps {
  open: boolean;
  connectionId: string;
  connectionName?: string;
  onClose: () => void;
}

/** Parse a port field, distinguishing "empty" from "invalid". */
function parsePortInput(value: string): { port?: number; valid: boolean } {
  const trimmed = value.trim();
  if (!trimmed) return { valid: true };
  if (!/^\d+$/.test(trimmed)) return { valid: false };
  const port = Number(trimmed);
  if (port < 1 || port > 65535) return { valid: false };
  return { port, valid: true };
}

export const PortForwardDialog: React.FC<PortForwardDialogProps> = ({
  open,
  connectionId,
  connectionName,
  onClose,
}) => {
  const { t } = useI18n('common');

  const [forwards, setForwards] = useState<PortForward[]>([]);
  const [remotePort, setRemotePort] = useState('');
  const [localPort, setLocalPort] = useState('');
  const [label, setLabel] = useState('');
  const [exposeOnLan, setExposeOnLan] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [detectedPorts, setDetectedPorts] = useState<RemoteListeningPort[] | null>(null);
  const [isDetecting, setIsDetecting] = useState(false);
  const remotePortInputRef = useRef<HTMLInputElement>(null);

  const refresh = useCallback(async () => {
    try {
      setForwards(await sshApi.listPortForwards(connectionId));
    } catch (err) {
      log.error('Failed to list port forwards', err);
    }
  }, [connectionId]);

  useEffect(() => {
    if (!open) return;
    void refresh();
    const timer = window.setInterval(() => {
      void refresh();
    }, REFRESH_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [open, refresh]);

  // A stale detection list would invite forwarding a port that has since gone
  // away, so it does not survive a close.
  useEffect(() => {
    if (open) return;
    setDetectedPorts(null);
    setError(null);
  }, [open]);

  const remotePortParsed = useMemo(() => parsePortInput(remotePort), [remotePort]);
  const localPortParsed = useMemo(() => parsePortInput(localPort), [localPort]);
  const canSubmit =
    !isStarting &&
    remotePortParsed.valid &&
    remotePortParsed.port !== undefined &&
    localPortParsed.valid;

  const handleAdd = useCallback(async () => {
    if (remotePortParsed.port === undefined || !localPortParsed.valid) return;
    setIsStarting(true);
    setError(null);
    try {
      await sshApi.startPortForward({
        connectionId,
        remotePort: remotePortParsed.port,
        localPort: localPortParsed.port,
        exposeOnLan,
        label: label.trim() || undefined,
      });
      setRemotePort('');
      setLocalPort('');
      setLabel('');
      remotePortInputRef.current?.focus();
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsStarting(false);
    }
  }, [
    connectionId,
    exposeOnLan,
    label,
    localPortParsed,
    refresh,
    remotePortParsed.port,
  ]);

  const handleStop = useCallback(
    async (forwardId: string) => {
      try {
        await sshApi.stopPortForward(forwardId);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [refresh]
  );

  const handleDetect = useCallback(async () => {
    setIsDetecting(true);
    setError(null);
    try {
      setDetectedPorts(await sshApi.listRemoteListeningPorts(connectionId));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setDetectedPorts(null);
    } finally {
      setIsDetecting(false);
    }
  }, [connectionId]);

  const localAddress = useCallback((forward: PortForward) => {
    const host =
      forward.localHost === '0.0.0.0'
        ? '127.0.0.1'
        : forward.localHost === '::'
          ? '[::1]'
          : forward.localHost;
    return `${host}:${forward.localPort}`;
  }, []);

  const handleOpen = useCallback(
    (forward: PortForward) => {
      void systemAPI
        .openExternal(`http://${localAddress(forward)}`)
        .catch((err) => log.error('Failed to open forwarded address', err));
    },
    [localAddress]
  );

  const handleCopy = useCallback(
    (forward: PortForward) => {
      void systemAPI
        .setClipboard(localAddress(forward))
        .catch((err) => log.error('Failed to copy forwarded address', err));
    },
    [localAddress]
  );

  const forwardedRemotePorts = useMemo(
    () => new Set(forwards.map((forward) => forward.remotePort)),
    [forwards]
  );

  return (
    <Modal
      isOpen={open}
      onClose={onClose}
      title={t('ssh.portForward.title')}
      size="large"
      showCloseButton
      testId="ssh-port-forward-dialog"
    >
      <div
        className="ssh-port-forward"
        data-bf-component="ssh-remote"
        data-bf-part="portForward"
      >
        <p className="ssh-port-forward__intro">
          {connectionName
            ? t('ssh.portForward.introNamed', { name: connectionName })
            : t('ssh.portForward.intro')}
        </p>

        <div
          className="ssh-port-forward__form"
          data-bf-component="ssh-remote"
          data-bf-part="portForwardForm"
        >
          <label className="ssh-port-forward__field">
            <span className="ssh-port-forward__field-label">
              {t('ssh.portForward.remotePortLabel')}
            </span>
            <Input
              ref={remotePortInputRef}
              inputSize="small"
              value={remotePort}
              placeholder="3000"
              error={!remotePortParsed.valid}
              onChange={(event) => setRemotePort(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && canSubmit) void handleAdd();
              }}
              data-testid="ssh-port-forward-remote-port"
            />
          </label>

          <label className="ssh-port-forward__field">
            <span className="ssh-port-forward__field-label">
              {t('ssh.portForward.localPortLabel')}
            </span>
            <Input
              inputSize="small"
              value={localPort}
              placeholder={t('ssh.portForward.localPortPlaceholder')}
              error={!localPortParsed.valid}
              onChange={(event) => setLocalPort(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && canSubmit) void handleAdd();
              }}
              data-testid="ssh-port-forward-local-port"
            />
          </label>

          <label className="ssh-port-forward__field ssh-port-forward__field--grow">
            <span className="ssh-port-forward__field-label">
              {t('ssh.portForward.labelLabel')}
            </span>
            <Input
              inputSize="small"
              value={label}
              placeholder={t('ssh.portForward.labelPlaceholder')}
              onChange={(event) => setLabel(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && canSubmit) void handleAdd();
              }}
            />
          </label>

          <Button
            variant="primary"
            size="small"
            disabled={!canSubmit}
            onClick={() => void handleAdd()}
            data-testid="ssh-port-forward-add"
          >
            <Plus size={14} aria-hidden="true" />
            {t('ssh.portForward.add')}
          </Button>

          <Button
            variant="secondary"
            size="small"
            disabled={isDetecting}
            onClick={() => void handleDetect()}
            data-testid="ssh-port-forward-detect"
          >
            {isDetecting ? (
              <RefreshCw size={14} className="ssh-port-forward__spin" aria-hidden="true" />
            ) : (
              <Radar size={14} aria-hidden="true" />
            )}
            {t('ssh.portForward.detect')}
          </Button>
        </div>

        <Checkbox
          size="small"
          checked={exposeOnLan}
          onChange={(event) => setExposeOnLan(event.target.checked)}
          label={t('ssh.portForward.exposeOnLan')}
          description={t('ssh.portForward.exposeOnLanHint')}
        />

        {error && (
          <div
            className="ssh-port-forward__error"
            role="alert"
            data-bf-component="ssh-remote"
            data-bf-part="portForwardError"
          >
            <AlertTriangle size={15} aria-hidden="true" />
            <span>{error}</span>
          </div>
        )}

        {detectedPorts && (
          <div className="ssh-port-forward__detected">
            <div className="ssh-port-forward__detected-title">
              {t('ssh.portForward.detectedTitle')}
            </div>
            {detectedPorts.length === 0 ? (
              <div className="ssh-port-forward__detected-empty">
                {t('ssh.portForward.detectedEmpty')}
              </div>
            ) : (
              <div className="ssh-port-forward__detected-list">
                {detectedPorts.map((port) => (
                  <button
                    key={`${port.port}-${port.bindAddress}`}
                    type="button"
                    className="ssh-port-forward__detected-item"
                    disabled={forwardedRemotePorts.has(port.port)}
                    title={
                      forwardedRemotePorts.has(port.port)
                        ? t('ssh.portForward.detectedAlreadyForwarded')
                        : t('ssh.portForward.detectedUse')
                    }
                    onClick={() => {
                      setRemotePort(String(port.port));
                      if (port.process && !label.trim()) setLabel(port.process);
                      remotePortInputRef.current?.focus();
                    }}
                  >
                    <span className="ssh-port-forward__detected-port">{port.port}</span>
                    {port.process && (
                      <span className="ssh-port-forward__detected-process">{port.process}</span>
                    )}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}

        <div
          className="ssh-port-forward__table"
          data-bf-component="ssh-remote"
          data-bf-part="portForwardTable"
        >
          {forwards.length === 0 ? (
            <Empty description={t('ssh.portForward.empty')} imageSize="small" />
          ) : (
            <table>
              <thead>
                <tr>
                  <th>{t('ssh.portForward.columnRemotePort')}</th>
                  <th>{t('ssh.portForward.columnLocalAddress')}</th>
                  <th>{t('ssh.portForward.columnLabel')}</th>
                  <th>{t('ssh.portForward.columnStatus')}</th>
                  <th aria-label={t('ssh.portForward.columnActions')} />
                </tr>
              </thead>
              <tbody>
                {forwards.map((forward) => (
                  <tr key={forward.id} data-testid="ssh-port-forward-row">
                    <td className="ssh-port-forward__cell-port">
                      {forward.remotePort}
                      {forward.remoteHost !== '127.0.0.1' && (
                        <span className="ssh-port-forward__cell-host">{forward.remoteHost}</span>
                      )}
                    </td>
                    <td className="ssh-port-forward__cell-address">
                      <code>{localAddress(forward)}</code>
                      {forward.requestedLocalPort !== undefined &&
                        forward.requestedLocalPort !== null && (
                          <span className="ssh-port-forward__moved">
                            {t('ssh.portForward.portMoved', {
                              requested: forward.requestedLocalPort,
                              bound: forward.localPort,
                            })}
                          </span>
                        )}
                      {forward.localHost === '0.0.0.0' && (
                        <span className="ssh-port-forward__lan">
                          {t('ssh.portForward.exposedOnLanBadge')}
                        </span>
                      )}
                    </td>
                    <td className="ssh-port-forward__cell-label">{forward.label ?? '—'}</td>
                    <td className="ssh-port-forward__cell-status">
                      {forward.lastError ? (
                        <span
                          className="ssh-port-forward__status ssh-port-forward__status--warn"
                          title={forward.lastError}
                        >
                          <AlertTriangle size={13} aria-hidden="true" />
                          {t('ssh.portForward.statusWarning')}
                        </span>
                      ) : (
                        <span className="ssh-port-forward__status">
                          {t('ssh.portForward.statusActive', {
                            active: forward.activeConnections,
                          })}
                        </span>
                      )}
                    </td>
                    <td className="ssh-port-forward__cell-actions">
                      <button
                        type="button"
                        title={t('ssh.portForward.openInBrowser')}
                        aria-label={t('ssh.portForward.openInBrowser')}
                        onClick={() => handleOpen(forward)}
                      >
                        <ExternalLink size={14} aria-hidden="true" />
                      </button>
                      <button
                        type="button"
                        title={t('ssh.portForward.copyAddress')}
                        aria-label={t('ssh.portForward.copyAddress')}
                        onClick={() => handleCopy(forward)}
                      >
                        <Copy size={14} aria-hidden="true" />
                      </button>
                      <button
                        type="button"
                        title={t('ssh.portForward.stop')}
                        aria-label={t('ssh.portForward.stop')}
                        onClick={() => void handleStop(forward.id)}
                        data-testid="ssh-port-forward-stop"
                      >
                        <X size={14} aria-hidden="true" />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </Modal>
  );
};

export default PortForwardDialog;
