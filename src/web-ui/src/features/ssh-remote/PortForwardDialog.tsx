/**
 * Remote Port Forwarding Dialog
 *
 * Manual mappings only: a forward exists because the user asked for it.
 *
 * Detection runs on open, though — knowing what the remote is listening on is
 * not the same as forwarding it, and making people type a port number they
 * would otherwise have to go look up is friction with nothing behind it. One
 * click on a detected port creates the mapping; the form below stays for the
 * cases detection cannot cover (a specific local port, a non-loopback remote
 * host).
 *
 * The table keeps the remote port and the local address in separate columns
 * because they routinely differ: the remote port is what the user thinks in,
 * the local port is an allocation that moves when the number is taken.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Modal, Button, Input, Checkbox } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createLogger } from '@/shared/utils/logger';
import { AlertTriangle, Copy, ExternalLink, Plus, RefreshCw, X, Check } from 'lucide-react';
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

/** Address to hand a browser. A wildcard bind is only reachable here via loopback. */
function localAddressOf(forward: PortForward): string {
  const host =
    forward.localHost === '0.0.0.0'
      ? '127.0.0.1'
      : forward.localHost === '::'
        ? '[::1]'
        : forward.localHost;
  return `${host}:${forward.localPort}`;
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
  const [busyPort, setBusyPort] = useState<number | null>(null);
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

  const detect = useCallback(async () => {
    setIsDetecting(true);
    try {
      setDetectedPorts(await sshApi.listRemoteListeningPorts(connectionId));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setDetectedPorts([]);
    } finally {
      setIsDetecting(false);
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

  // Detect once per opening. Re-detecting on every poll would fight the user's
  // cursor, and a listing captured minutes ago is worse than no listing.
  useEffect(() => {
    if (!open) return;
    void detect();
  }, [open, detect]);

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

  const startForward = useCallback(
    async (request: {
      remotePort: number;
      localPort?: number;
      label?: string;
      exposeOnLan?: boolean;
    }) => {
      setError(null);
      try {
        await sshApi.startPortForward({
          connectionId,
          remotePort: request.remotePort,
          localPort: request.localPort,
          exposeOnLan: request.exposeOnLan ?? false,
          label: request.label,
        });
        await refresh();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      }
    },
    [connectionId, refresh]
  );

  const handleAdd = useCallback(async () => {
    if (remotePortParsed.port === undefined || !localPortParsed.valid) return;
    setIsStarting(true);
    const ok = await startForward({
      remotePort: remotePortParsed.port,
      localPort: localPortParsed.port,
      label: label.trim() || undefined,
      exposeOnLan,
    });
    setIsStarting(false);
    if (ok) {
      setRemotePort('');
      setLocalPort('');
      setLabel('');
      remotePortInputRef.current?.focus();
    }
  }, [exposeOnLan, label, localPortParsed, remotePortParsed.port, startForward]);

  /** One click on a detected port is still the user asking for the mapping. */
  const handleForwardDetected = useCallback(
    async (port: RemoteListeningPort) => {
      setBusyPort(port.port);
      await startForward({ remotePort: port.port, label: port.process ?? undefined });
      setBusyPort(null);
    },
    [startForward]
  );

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

  const handleOpen = useCallback((forward: PortForward) => {
    void systemAPI
      .openExternal(`http://${localAddressOf(forward)}`)
      .catch((err) => log.error('Failed to open forwarded address', err));
  }, []);

  const handleCopy = useCallback((forward: PortForward) => {
    void systemAPI
      .setClipboard(localAddressOf(forward))
      .catch((err) => log.error('Failed to copy forwarded address', err));
  }, []);

  const forwardedRemotePorts = useMemo(
    () => new Set(forwards.map((forward) => forward.remotePort)),
    [forwards]
  );

  return (
    <Modal
      isOpen={open}
      onClose={onClose}
      title={t('ssh.portForward.title')}
      titleExtra={
        connectionName ? (
          <span className="ssh-port-forward__target">{connectionName}</span>
        ) : undefined
      }
      size="medium"
      showCloseButton
      testId="ssh-port-forward-dialog"
    >
      <div className="ssh-port-forward" data-bf-component="ssh-remote" data-bf-part="portForward">
        {/* Discovery first: it answers "which port?" without making anyone look it up. */}
        <section className="ssh-port-forward__section">
          <header className="ssh-port-forward__section-head">
            <h4>{t('ssh.portForward.detectedTitle')}</h4>
            <button
              type="button"
              className="ssh-port-forward__icon-button"
              onClick={() => void detect()}
              disabled={isDetecting}
              title={t('ssh.portForward.detect')}
              aria-label={t('ssh.portForward.detect')}
              data-testid="ssh-port-forward-detect"
            >
              <RefreshCw
                size={13}
                className={isDetecting ? 'ssh-port-forward__spin' : undefined}
                aria-hidden="true"
              />
            </button>
          </header>

          {isDetecting && detectedPorts === null ? (
            <p className="ssh-port-forward__muted">{t('ssh.portForward.detecting')}</p>
          ) : detectedPorts && detectedPorts.length > 0 ? (
            <div className="ssh-port-forward__chips">
              {detectedPorts.map((port) => {
                const alreadyForwarded = forwardedRemotePorts.has(port.port);
                return (
                  <button
                    key={`${port.port}-${port.bindAddress}`}
                    type="button"
                    className="ssh-port-forward__chip"
                    data-state={alreadyForwarded ? 'forwarded' : 'idle'}
                    disabled={alreadyForwarded || busyPort === port.port}
                    title={
                      alreadyForwarded
                        ? t('ssh.portForward.detectedAlreadyForwarded')
                        : t('ssh.portForward.detectedUse')
                    }
                    onClick={() => void handleForwardDetected(port)}
                    data-testid="ssh-port-forward-chip"
                    data-port={port.port}
                  >
                    {alreadyForwarded ? (
                      <Check size={12} aria-hidden="true" />
                    ) : (
                      <Plus size={12} aria-hidden="true" />
                    )}
                    <span className="ssh-port-forward__chip-port">{port.port}</span>
                    {port.process && (
                      <span className="ssh-port-forward__chip-process">{port.process}</span>
                    )}
                  </button>
                );
              })}
            </div>
          ) : (
            <p className="ssh-port-forward__muted">{t('ssh.portForward.detectedEmpty')}</p>
          )}
        </section>

        {error && (
          <div
            className="ssh-port-forward__error"
            role="alert"
            data-bf-component="ssh-remote"
            data-bf-part="portForwardError"
          >
            <AlertTriangle size={14} aria-hidden="true" />
            <span>{error}</span>
          </div>
        )}

        <section className="ssh-port-forward__section">
          <header className="ssh-port-forward__section-head">
            <h4>{t('ssh.portForward.activeTitle')}</h4>
          </header>

          {forwards.length === 0 ? (
            <p className="ssh-port-forward__muted">{t('ssh.portForward.empty')}</p>
          ) : (
            <div
              className="ssh-port-forward__table"
              data-bf-component="ssh-remote"
              data-bf-part="portForwardTable"
            >
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
                        <code>{localAddressOf(forward)}</code>
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
                            <AlertTriangle size={12} aria-hidden="true" />
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
                          className="ssh-port-forward__icon-button"
                          title={t('ssh.portForward.openInBrowser')}
                          aria-label={t('ssh.portForward.openInBrowser')}
                          onClick={() => handleOpen(forward)}
                        >
                          <ExternalLink size={13} aria-hidden="true" />
                        </button>
                        <button
                          type="button"
                          className="ssh-port-forward__icon-button"
                          title={t('ssh.portForward.copyAddress')}
                          aria-label={t('ssh.portForward.copyAddress')}
                          onClick={() => handleCopy(forward)}
                        >
                          <Copy size={13} aria-hidden="true" />
                        </button>
                        <button
                          type="button"
                          className="ssh-port-forward__icon-button ssh-port-forward__icon-button--danger"
                          title={t('ssh.portForward.stop')}
                          aria-label={t('ssh.portForward.stop')}
                          onClick={() => void handleStop(forward.id)}
                          data-testid="ssh-port-forward-stop"
                        >
                          <X size={13} aria-hidden="true" />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        {/* The escape hatch: a specific local port, or a host detection cannot see. */}
        <section className="ssh-port-forward__section ssh-port-forward__section--manual">
          <header className="ssh-port-forward__section-head">
            <h4>{t('ssh.portForward.manualTitle')}</h4>
          </header>

          <div
            className="ssh-port-forward__form"
            data-bf-component="ssh-remote"
            data-bf-part="portForwardForm"
          >
            <label className="ssh-port-forward__field">
              <span>{t('ssh.portForward.remotePortLabel')}</span>
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
              <span>{t('ssh.portForward.localPortLabel')}</span>
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
              <span>{t('ssh.portForward.labelLabel')}</span>
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
              variant="secondary"
              size="small"
              disabled={!canSubmit}
              onClick={() => void handleAdd()}
              data-testid="ssh-port-forward-add"
            >
              {t('ssh.portForward.add')}
            </Button>
          </div>

          <Checkbox
            size="small"
            checked={exposeOnLan}
            onChange={(event) => setExposeOnLan(event.target.checked)}
            label={t('ssh.portForward.exposeOnLan')}
            title={t('ssh.portForward.exposeOnLanHint')}
          />
        </section>
      </div>
    </Modal>
  );
};

export default PortForwardDialog;
