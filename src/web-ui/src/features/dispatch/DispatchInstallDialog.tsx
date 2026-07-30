import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  Alert,
  Button,
  Input,
  Modal,
  confirmWarning,
} from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import {
  Check,
  Loader2,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  ShieldQuestion,
} from 'lucide-react';
import { dispatchApi } from './dispatchApi';
import type {
  DispatchApprovalPolicy,
  DispatchInstallStart,
  DispatchSelection,
  DispatchSshProbe,
  DispatchTargetOption,
  DispatchWorkspaceDeliveryRequest,
} from './types';
import {
  BASE_DISPATCH_CAPABILITIES,
  DISPATCH_PROTOCOL_VERSION,
  isDispatchWorkspaceReady,
} from './dispatchPreflight';
import './DispatchInstallDialog.scss';

const log = createLogger('DispatchInstallDialog');
const INSTALL_POLL_INTERVAL_MS = 1200;

interface ActiveInstall {
  connectionId: string;
  generation: number;
  phase: 'starting' | 'polling';
}

function approvalCapability(policy: DispatchApprovalPolicy | null): string | null {
  if (policy === 'auto') return 'approval_auto';
  if (policy === 'reject-and-report') return 'approval_reject_and_report';
  if (policy === 'remote') return 'approval_remote';
  return null;
}

interface DispatchInstallDialogProps {
  open: boolean;
  target: DispatchTargetOption | null;
  sourceWorkspacePath?: string;
  onClose: () => void;
  onReady: (selection: DispatchSelection) => void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const DispatchInstallDialog: React.FC<DispatchInstallDialogProps> = ({
  open,
  target,
  sourceWorkspacePath,
  onClose,
  onReady,
}) => {
  const { t } = useI18n('common');
  const [workspacePath, setWorkspacePath] = useState('');
  const [approvalPolicy, setApprovalPolicy] = useState<DispatchApprovalPolicy | null>(null);
  const [deliveryKind, setDeliveryKind] = useState<'existing' | 'snapshot-exact'>('existing');
  const [sensitiveFilesConfirmed, setSensitiveFilesConfirmed] = useState(false);
  const [probe, setProbe] = useState<DispatchSshProbe | null>(null);
  const [probedWorkspaceInput, setProbedWorkspaceInput] = useState<string | null>(null);
  const [probing, setProbing] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installStart, setInstallStart] = useState<DispatchInstallStart | null>(null);
  const [installOutput, setInstallOutput] = useState('');
  const [error, setError] = useState<string | null>(null);
  const generationRef = useRef(0);
  const activeInstallRef = useRef<ActiveInstall | null>(null);
  const workspacePathRef = useRef(workspacePath);
  workspacePathRef.current = workspacePath;

  const connectionId = target?.connectionId?.trim() ?? '';
  const deviceId = target?.deviceId?.trim() ?? '';
  const targetId = target?.kind === 'device' ? deviceId : connectionId;
  const deliveryKindRef = useRef(deliveryKind);
  deliveryKindRef.current = deliveryKind;

  const runProbe = useCallback(async (pathOverride?: string) => {
    if (!targetId || !target || target.kind === 'local') return;
    const path = deliveryKindRef.current === 'existing'
      ? (pathOverride ?? workspacePathRef.current).trim()
      : '';
    const generation = ++generationRef.current;
    setProbing(true);
    setError(null);
    try {
      const result = await dispatchApi.probeTarget(
        target.kind === 'device'
          ? { kind: 'device', deviceId: targetId, workspacePath: path }
          : { kind: 'ssh', connectionId: targetId, workspacePath: path },
      );
      if (generation === generationRef.current) {
        setProbe(result);
        setProbedWorkspaceInput(path);
      }
    } catch (nextError) {
      if (generation === generationRef.current) {
        setProbe(null);
        setProbedWorkspaceInput(null);
        setError(errorMessage(nextError));
      }
    } finally {
      if (generation === generationRef.current) {
        setProbing(false);
      }
    }
  }, [target, targetId]);

  useEffect(() => {
    if (!open || !targetId) return;
    const initialPath = target?.defaultWorkspace?.trim() ?? '';
    setWorkspacePath(initialPath);
    setApprovalPolicy(null);
    setDeliveryKind('existing');
    setSensitiveFilesConfirmed(false);
    setProbe(null);
    setProbedWorkspaceInput(null);
    setInstallStart(null);
    setInstallOutput('');
    setInstalling(false);
    setError(null);
    void runProbe(initialPath);
  }, [open, runProbe, target?.defaultWorkspace, targetId]);

  const clearActiveInstall = useCallback((generation: number) => {
    if (activeInstallRef.current?.generation === generation) {
      activeInstallRef.current = null;
    }
  }, []);

  const cancelActiveInstall = useCallback(() => {
    const activeInstall = activeInstallRef.current;
    if (!activeInstall) return;
    activeInstallRef.current = null;
    void dispatchApi.installCliCancel(activeInstall.connectionId).catch(nextError => {
      log.warn('Failed to cancel SSH CLI installation', { error: nextError });
    });
  }, []);

  const invalidateInstallLifecycle = useCallback(() => {
    generationRef.current += 1;
    cancelActiveInstall();
  }, [cancelActiveInstall]);

  useEffect(() => {
    if (!open || !targetId) return;
    return invalidateInstallLifecycle;
  }, [invalidateInstallLifecycle, open, targetId]);

  useEffect(() => {
    if (!open || !targetId) return;
    void runProbe(deliveryKind === 'existing' ? workspacePathRef.current : '');
  }, [deliveryKind, open, runProbe, targetId]);

  const pollInstallation = useCallback(async (generation: number) => {
    if (!connectionId) return;
    let cursor = 0;
    if (
      generation !== generationRef.current ||
      activeInstallRef.current?.generation !== generation
    ) {
      return;
    }
    activeInstallRef.current = {
      connectionId,
      generation,
      phase: 'polling',
    };
    setInstalling(true);
    try {
      while (generation === generationRef.current) {
        const result = await dispatchApi.installCliPoll(connectionId, cursor);
        if (generation !== generationRef.current) return;
        cursor = result.cursor;
        if (result.output) {
          setInstallOutput(previous => previous + result.output);
        }
        if (result.status === 'succeeded') {
          clearActiveInstall(generation);
          setInstalling(false);
          await runProbe();
          return;
        }
        if (result.status === 'failed') {
          clearActiveInstall(generation);
          setInstalling(false);
          setError(t('dispatch.installFailed'));
          return;
        }
        await new Promise(resolve => window.setTimeout(resolve, INSTALL_POLL_INTERVAL_MS));
      }
      clearActiveInstall(generation);
    } catch (nextError) {
      if (generation === generationRef.current) {
        clearActiveInstall(generation);
        setInstalling(false);
        setError(errorMessage(nextError));
      }
    }
  }, [clearActiveInstall, connectionId, runProbe, t]);

  const startInstallation = useCallback(async () => {
    if (!connectionId || !probe?.release) return;
    const release = probe.release;
    const generation = ++generationRef.current;
    const confirmed = await confirmWarning(
      t('dispatch.installConfirmTitle'),
      t('dispatch.installConfirmMessage', {
        version: release.version,
        url: release.url,
        sha256: release.sha256,
      }),
      {
        confirmText: t('dispatch.installConfirm'),
        cancelText: t('dispatch.cancel'),
      },
    );
    if (!confirmed || generation !== generationRef.current) return;

    setError(null);
    setInstallOutput('');
    setInstalling(true);
    activeInstallRef.current = {
      connectionId,
      generation,
      phase: 'starting',
    };
    try {
      const started = await dispatchApi.installCliStart(connectionId, release);
      if (generation !== generationRef.current) {
        clearActiveInstall(generation);
        // Closing while the start request is in flight may race with a first
        // cancel that reaches the target before the installer exists. Cancel
        // again after the late start acknowledgement to avoid an orphan.
        await dispatchApi.installCliCancel(connectionId).catch(nextError => {
          log.warn('Failed to cancel stale SSH CLI installation', { error: nextError });
        });
        return;
      }
      setInstallStart(started);
      void pollInstallation(generation);
    } catch (nextError) {
      clearActiveInstall(generation);
      if (generation === generationRef.current) {
        setInstalling(false);
        setError(errorMessage(nextError));
      }
    }
  }, [clearActiveInstall, connectionId, pollInstallation, probe?.release, t]);

  const close = useCallback(() => {
    invalidateInstallLifecycle();
    setInstalling(false);
    onClose();
  }, [invalidateInstallLifecycle, onClose]);

  const protocol = probe?.protocol;
  const workspace = protocol?.workspace;
  const selectedApprovalCapability = approvalCapability(approvalPolicy);
  const requiredCapabilities = [
    ...BASE_DISPATCH_CAPABILITIES,
    ...(selectedApprovalCapability ? [selectedApprovalCapability] : []),
    ...(deliveryKind === 'snapshot-exact'
      ? ['workspace_snapshot_exact', 'workspace_snapshot_chunked']
      : []),
  ];
  const missingCapabilities = protocol
    ? requiredCapabilities.filter(capability => !protocol.capabilities.includes(capability))
    : requiredCapabilities;
  const protocolCompatible =
    protocol?.protocolVersion === DISPATCH_PROTOCOL_VERSION &&
    missingCapabilities.length === 0;
  const cliReady =
    !!probe?.cliInstalled &&
    !!protocol &&
    !probe.protocolError &&
    protocolCompatible;
  const workspaceReady = deliveryKind === 'snapshot-exact'
    ? !!sourceWorkspacePath?.trim() && sensitiveFilesConfirmed
    : isDispatchWorkspaceReady(workspacePath, workspace, probedWorkspaceInput ?? undefined);
  const modelReady = protocol?.modelConfigured === true;
  const ready = cliReady && workspaceReady && modelReady && approvalPolicy !== null;

  const confirmTarget = () => {
    if (
      !target
      || target.kind === 'local'
      || !targetId
      || !approvalPolicy
      || !ready
    ) return;
    const normalizedPath = deliveryKind === 'existing'
      ? workspace?.path?.trim() || workspacePath.trim()
      : '';
    const workspaceDelivery: DispatchWorkspaceDeliveryRequest =
      deliveryKind === 'snapshot-exact'
        ? {
            kind: 'snapshot-exact',
            sourceWorkspacePath: sourceWorkspacePath!.trim(),
            sensitiveFilesConfirmed: true,
          }
        : { kind: 'existing' };
    const request = target.kind === 'device'
      ? {
          kind: 'device' as const,
          deviceId: targetId,
          workspacePath: normalizedPath,
        }
      : {
          kind: 'ssh' as const,
          connectionId: targetId,
          workspacePath: normalizedPath,
        };
    onReady({
      request,
      target: {
        ...request,
        workspacePath: normalizedPath,
        displayName: target.displayName,
      },
      workspaceDelivery,
      approvalPolicy,
    });
  };

  return (
    <Modal
      isOpen={open}
      onClose={close}
      title={t('dispatch.configureTitle', { target: target?.displayName ?? '' })}
      size="medium"
      closeOnOverlayClick
      showCloseButton
      testId="dispatch-install-dialog"
    >
      <div className="dispatch-install-dialog">
        {error ? (
          <Alert type="error" message={error} closable onClose={() => setError(null)} />
        ) : null}

        <fieldset className="dispatch-install-dialog__delivery" disabled={installing}>
          <legend>{t('dispatch.deliveryTitle')}</legend>
          <button
            type="button"
            role="radio"
            aria-checked={deliveryKind === 'existing'}
            data-selected={deliveryKind === 'existing'}
            onClick={() => setDeliveryKind('existing')}
          >
            <span>
              <strong>{t('dispatch.deliveryExisting')}</strong>
              <small>{t('dispatch.deliveryExistingDescription')}</small>
            </span>
            {deliveryKind === 'existing' ? <Check size={14} /> : null}
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={deliveryKind === 'snapshot-exact'}
            data-selected={deliveryKind === 'snapshot-exact'}
            disabled={!sourceWorkspacePath?.trim()}
            onClick={() => setDeliveryKind('snapshot-exact')}
          >
            <span>
              <strong>{t('dispatch.deliverySnapshot')}</strong>
              <small>
                {sourceWorkspacePath?.trim()
                  ? t('dispatch.deliverySnapshotDescription')
                  : t('dispatch.deliverySnapshotUnavailable')}
              </small>
            </span>
            {deliveryKind === 'snapshot-exact' ? <Check size={14} /> : null}
          </button>
        </fieldset>

        {deliveryKind === 'existing' ? (
          <label className="dispatch-install-dialog__field">
            <span>{t('dispatch.workspacePath')}</span>
            <div className="dispatch-install-dialog__field-row">
              <Input
                value={workspacePath}
                placeholder={t('dispatch.workspacePlaceholder')}
                disabled={installing}
                onChange={event => setWorkspacePath(event.target.value)}
                onKeyDown={event => {
                  if (event.key === 'Enter') {
                    void runProbe();
                  }
                }}
              />
              <Button
                variant="secondary"
                size="small"
                disabled={probing || installing || !targetId}
                onClick={() => void runProbe()}
              >
                {probing ? <Loader2 size={14} className="dispatch-install-dialog__spin" /> : <RefreshCw size={14} />}
                {t('dispatch.check')}
              </Button>
            </div>
          </label>
        ) : (
          <div className="dispatch-install-dialog__snapshot-warning">
            <strong>{t('dispatch.snapshotSource')}</strong>
            <code>{sourceWorkspacePath}</code>
            <span>{t('dispatch.snapshotWarning')}</span>
            <label>
              <input
                type="checkbox"
                checked={sensitiveFilesConfirmed}
                onChange={event => setSensitiveFilesConfirmed(event.target.checked)}
              />
              {t('dispatch.snapshotConfirm')}
            </label>
          </div>
        )}

        {probe ? (
          <div className="dispatch-install-dialog__checks">
            <div data-state={cliReady ? 'ok' : 'blocked'}>
              <span>{t('dispatch.cliStatus')}</span>
              <strong>
                {cliReady
                  ? t('dispatch.cliReady', { version: protocol?.cliVersion })
                  : probe.cliInstalled && protocol
                    ? t('dispatch.cliIncompatible', {
                        details: protocol.protocolVersion !== DISPATCH_PROTOCOL_VERSION
                          ? t('dispatch.protocolVersionMismatch', {
                              expected: DISPATCH_PROTOCOL_VERSION,
                              actual: protocol.protocolVersion,
                            })
                          : missingCapabilities.join(', '),
                      })
                    : t('dispatch.cliMissing')}
              </strong>
            </div>
            {deliveryKind === 'existing' && workspacePath.trim() ? (
              <div data-state={workspaceReady ? 'ok' : 'blocked'}>
                <span>{t('dispatch.workspaceStatus')}</span>
                <strong>
                  {workspaceReady
                    ? workspace?.isGitRepository
                      ? t('dispatch.workspaceGit', {
                          branch: workspace.branch || t('dispatch.unknownBranch'),
                          dirty: workspace.dirty ? t('dispatch.dirty') : t('dispatch.clean'),
                        })
                      : t('dispatch.workspaceDirectory')
                    : t('dispatch.workspaceMissing')}
                </strong>
              </div>
            ) : null}
            {deliveryKind === 'existing' && workspaceReady && workspace?.isGitRepository &&
            (typeof workspace.ahead === 'number' || typeof workspace.behind === 'number') ? (
              <div data-state="ok">
                <span>{t('dispatch.upstreamStatus')}</span>
                <strong>
                  {t('dispatch.upstreamCounts', {
                    ahead: workspace.ahead ?? 0,
                    behind: workspace.behind ?? 0,
                  })}
                </strong>
              </div>
            ) : null}
            <div data-state={modelReady ? 'ok' : 'blocked'}>
              <span>{t('dispatch.modelStatus')}</span>
              <strong>
                {modelReady
                  ? t('dispatch.modelReady', { model: protocol?.defaultModel || t('dispatch.modelAutomatic') })
                  : protocol?.modelDiagnostic || t('dispatch.modelMissing')}
              </strong>
            </div>
          </div>
        ) : null}

        {target?.kind === 'ssh' && !cliReady && probe?.release ? (
          <div className="dispatch-install-dialog__install-card">
            <div>
              <strong>{t('dispatch.installRequired')}</strong>
              <span>{t('dispatch.installDescription')}</span>
            </div>
            <dl>
              <div><dt>{t('dispatch.version')}</dt><dd>{probe.release.version}</dd></div>
              <div><dt>{t('dispatch.downloadUrl')}</dt><dd>{probe.release.url}</dd></div>
              <div><dt>SHA256</dt><dd>{probe.release.sha256}</dd></div>
            </dl>
            <Button
              variant="primary"
              size="small"
              disabled={installing || !probe.installSupported}
              onClick={() => void startInstallation()}
            >
              {installing ? <Loader2 size={14} className="dispatch-install-dialog__spin" /> : null}
              {installing ? t('dispatch.installing') : t('dispatch.installConfirm')}
            </Button>
          </div>
        ) : null}

        {probe?.installError ? (
          <Alert type="warning" message={probe.installError} />
        ) : null}

        {installStart || installOutput ? (
          <pre className="dispatch-install-dialog__output" aria-label={t('dispatch.installOutput')}>
            {installOutput || t('dispatch.installWaiting')}
          </pre>
        ) : null}

        <fieldset className="dispatch-install-dialog__approval" disabled={installing}>
          <legend>{t('dispatch.approvalTitle')}</legend>
          <span className="dispatch-install-dialog__approval-hint">
            {t('dispatch.approvalHint')}
          </span>
          <button
            type="button"
            role="radio"
            aria-checked={approvalPolicy === 'reject-and-report'}
            data-selected={approvalPolicy === 'reject-and-report'}
            onClick={() => setApprovalPolicy('reject-and-report')}
          >
            <ShieldAlert size={16} />
            <span>
              <strong>{t('dispatch.approvalReject')}</strong>
              <small>{t('dispatch.approvalRejectDescription')}</small>
            </span>
            {approvalPolicy === 'reject-and-report' ? <Check size={14} /> : null}
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={approvalPolicy === 'remote'}
            data-selected={approvalPolicy === 'remote'}
            onClick={() => setApprovalPolicy('remote')}
          >
            <ShieldQuestion size={16} />
            <span>
              <strong>{t('dispatch.approvalRemote')}</strong>
              <small>{t('dispatch.approvalRemoteDescription')}</small>
            </span>
            {approvalPolicy === 'remote' ? <Check size={14} /> : null}
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={approvalPolicy === 'auto'}
            data-selected={approvalPolicy === 'auto'}
            onClick={() => setApprovalPolicy('auto')}
          >
            <ShieldCheck size={16} />
            <span>
              <strong>{t('dispatch.approvalAuto')}</strong>
              <small>{t('dispatch.approvalAutoDescription')}</small>
            </span>
            {approvalPolicy === 'auto' ? <Check size={14} /> : null}
          </button>
        </fieldset>

        <div className="dispatch-install-dialog__actions">
          <Button variant="secondary" size="small" onClick={close}>
            {t('dispatch.cancel')}
          </Button>
          <Button variant="primary" size="small" disabled={!ready || installing} onClick={confirmTarget}>
            {t('dispatch.useTarget')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};
