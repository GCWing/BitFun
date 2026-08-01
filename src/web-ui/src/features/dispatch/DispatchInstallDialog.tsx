import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  Alert,
  Button,
  Modal,
  confirmWarning,
} from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import {
  Check,
  Loader2,
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
} from './types';
import {
  BASE_DISPATCH_CAPABILITIES,
  DISPATCH_PROTOCOL_VERSION,
} from './dispatchPreflight';
import {
  compareDispatchModels,
  syncableLocalModelIds,
} from './dispatchModelParity';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { gitAPI } from '@/infrastructure/api/service-api/GitAPI';
import { configManager } from '@/infrastructure/config';
import { getModelDisplayName } from '@/infrastructure/config/services/modelConfigs';
import type { AIModelConfig } from '@/infrastructure/config/types';
import type { WorktreeSettings } from '@/infrastructure/api/service-api/WorktreeAPI';
import './DispatchInstallDialog.scss';

const log = createLogger('DispatchInstallDialog');
const INSTALL_POLL_INTERVAL_MS = 1200;
const DIALOG_TITLE_ID = 'dispatch-install-dialog-title';

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
  const [approvalPolicy, setApprovalPolicy] = useState<DispatchApprovalPolicy | null>(null);
  const [includeUncommitted, setIncludeUncommitted] = useState(false);
  const [baseRef, setBaseRef] = useState('HEAD');
  const [baseRefError, setBaseRefError] = useState<string | null>(null);
  const [validatingBaseRef, setValidatingBaseRef] = useState(false);
  const [worktreeSettingsLoading, setWorktreeSettingsLoading] = useState(true);
  const [probe, setProbe] = useState<DispatchSshProbe | null>(null);
  const [probing, setProbing] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [syncingModel, setSyncingModel] = useState(false);
  const [installStart, setInstallStart] = useState<DispatchInstallStart | null>(null);
  const [installOutput, setInstallOutput] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [localModels, setLocalModels] = useState<AIModelConfig[] | null>(null);
  const generationRef = useRef(0);
  const activeInstallRef = useRef<ActiveInstall | null>(null);
  const includeUncommittedTouchedRef = useRef(false);

  const connectionId = target?.connectionId?.trim() ?? '';
  const deviceId = target?.deviceId?.trim() ?? '';
  const targetId = target?.kind === 'device' ? deviceId : connectionId;

  const runProbe = useCallback(async () => {
    if (!targetId || !target || target.kind === 'local') return;
    // The target's own directories are irrelevant now: dispatch checks out its
    // own worktree there, so the probe only reports CLI and model readiness.
    const path = '';
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
      }
    } catch (nextError) {
      if (generation === generationRef.current) {
        setProbe(null);
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
    setApprovalPolicy(null);
    includeUncommittedTouchedRef.current = false;
    setIncludeUncommitted(false);
    setBaseRef('HEAD');
    setBaseRefError(null);
    setValidatingBaseRef(false);
    setWorktreeSettingsLoading(true);
    setProbe(null);
    setInstallStart(null);
    setInstallOutput('');
    setInstalling(false);
    setSyncingModel(false);
    setError(null);
    void runProbe();
  }, [open, runProbe, targetId]);

  // Reload on every open: the model catalog can change in settings while this
  // dialog is closed, and a stale local list would report a false divergence.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void configManager.getConfig<AIModelConfig[]>('ai.models')
      .then(models => {
        if (!cancelled) setLocalModels(Array.isArray(models) ? models : []);
      })
      .catch(nextError => {
        // Parity is advisory. Losing it degrades the readout to the target's
        // own facts rather than blocking the dialog.
        log.warn('Failed to read local model configuration for dispatch parity', {
          error: nextError,
        });
        if (!cancelled) setLocalModels(null);
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  // Dispatch uses the same baseline creation path as the regular worktree
  // control, so its initial copy-local-changes choice follows that setting.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setWorktreeSettingsLoading(true);
    void configAPI.getConfig('app.worktrees', { skipRetryOnNotFound: true })
      .then(settings => {
        if (!cancelled && !includeUncommittedTouchedRef.current) {
          const configured = settings as Partial<WorktreeSettings> | undefined;
          setIncludeUncommitted(configured?.copyLocalChanges === true);
        }
      })
      .catch(nextError => {
        log.warn('Failed to read worktree settings for dispatch', {
          error: nextError,
        });
        if (!cancelled && !includeUncommittedTouchedRef.current) {
          setIncludeUncommitted(false);
        }
      })
      .finally(() => {
        if (!cancelled) setWorktreeSettingsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, targetId]);

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

  // Same lifecycle as a release install — it shares the target-side driver,
  // log, and poll/cancel machinery, so only the start call differs.
  const startSourceBuild = useCallback(async () => {
    if (!connectionId) return;
    const generation = ++generationRef.current;
    const confirmed = await confirmWarning(
      t('dispatch.sourceBuildConfirmTitle'),
      t('dispatch.sourceBuildConfirmMessage'),
      {
        confirmText: t('dispatch.sourceBuildConfirm'),
        cancelText: t('dispatch.cancel'),
      },
    );
    if (!confirmed || generation !== generationRef.current) return;

    setError(null);
    setInstallOutput('');
    setInstalling(true);
    activeInstallRef.current = { connectionId, generation, phase: 'starting' };
    try {
      const started = await dispatchApi.installCliSourceStart(connectionId);
      if (generation !== generationRef.current) {
        clearActiveInstall(generation);
        await dispatchApi.installCliCancel(connectionId).catch(nextError => {
          log.warn('Failed to cancel stale SSH CLI source build', { error: nextError });
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
  }, [clearActiveInstall, connectionId, pollInstallation, t]);

  const syncModelConfiguration = useCallback(async () => {
    if (!connectionId) return;
    const generation = generationRef.current;
    const confirmed = await confirmWarning(
      t('dispatch.syncModelConfirmTitle'),
      t('dispatch.syncModelConfirmMessage'),
      {
        confirmText: t('dispatch.syncModelConfirm'),
        cancelText: t('dispatch.cancel'),
      },
    );
    if (!confirmed || generation !== generationRef.current) return;
    setSyncingModel(true);
    setError(null);
    try {
      await dispatchApi.syncModelConfig(connectionId);
    } catch (nextError) {
      if (generation === generationRef.current) {
        setSyncingModel(false);
        setError(errorMessage(nextError));
      }
      return;
    }
    if (generation !== generationRef.current) return;
    // runProbe advances the generation, so leave the syncing state first.
    setSyncingModel(false);
    await runProbe();
  }, [connectionId, runProbe, t]);

  const close = useCallback(() => {
    invalidateInstallLifecycle();
    setInstalling(false);
    setSyncingModel(false);
    onClose();
  }, [invalidateInstallLifecycle, onClose]);

  const protocol = probe?.protocol;
  const selectedApprovalCapability = approvalCapability(approvalPolicy);
  const requiredCapabilities = [
    ...BASE_DISPATCH_CAPABILITIES,
    ...(selectedApprovalCapability ? [selectedApprovalCapability] : []),
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
  const workspaceReady = !!sourceWorkspacePath?.trim();
  const modelReady = protocol?.modelConfigured === true;
  /**
   * A missing CLI no longer blocks target selection: submitting installs the
   * signed release automatically. Model readiness cannot be checked until that
   * CLI exists, so it stays unverified here and submit reports it instead.
   */
  const installPending =
    !cliReady
    && target?.kind === 'ssh'
    && !!probe?.installSupported
    && !probe?.prebuiltIncompatible;
  const ready =
    approvalPolicy !== null
    && workspaceReady
    && (cliReady ? modelReady : installPending);

  const targetModelCount = protocol?.availableModels?.length ?? 0;
  const modelParity = compareDispatchModels(
    syncableLocalModelIds(localModels),
    protocol?.availableModels,
  );
  // The probe carries ids, which name nothing a user recognizes. Resolve the
  // target's default through the local catalog when the two agree; when they
  // do not, the id would be misleading anyway and the count is the actionable
  // fact.
  const targetDefaultModelLabel = (() => {
    const id = protocol?.defaultModel?.trim();
    if (!id) return t('dispatch.modelAutomatic');
    const local = localModels?.find(model => model.id?.trim() === id);
    return local ? getModelDisplayName(local) : id;
  })();

  const confirmTarget = async () => {
    if (
      !target
      || target.kind === 'local'
      || !targetId
      || !approvalPolicy
      || !ready
    ) return;
    const normalizedSourcePath = sourceWorkspacePath?.trim() || '';
    const normalizedBaseRef = baseRef.trim() || 'HEAD';
    const generation = generationRef.current;
    setValidatingBaseRef(true);
    setBaseRefError(null);
    try {
      await gitAPI.resolveRevision(normalizedSourcePath, normalizedBaseRef);
    } catch (nextError) {
      if (generation === generationRef.current) {
        log.warn('Failed to resolve dispatch base revision', {
          repositoryPath: normalizedSourcePath,
          revision: normalizedBaseRef,
          error: nextError,
        });
        setBaseRefError(t('dispatch.baseRefInvalid', { ref: normalizedBaseRef }));
      }
      return;
    } finally {
      if (generation === generationRef.current) {
        setValidatingBaseRef(false);
      }
    }
    if (generation !== generationRef.current) return;
    // The target chooses where its worktree lands, so nothing is sent here.
    const normalizedPath = '';
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
      includeUncommitted,
      baseRef: normalizedBaseRef,
      approvalPolicy,
      availableModels: protocol?.availableModels,
      defaultModel: protocol?.defaultModel,
    });
  };

  const sourceBuild = probe?.sourceBuild;

  return (
    <Modal
      isOpen={open}
      onClose={close}
      size="medium"
      closeOnOverlayClick
      showCloseButton
      // The dialog renders its own heading, so point the modal's label at it
      // rather than at the chrome title it no longer uses.
      ariaLabelledBy={DIALOG_TITLE_ID}
      testId="dispatch-install-dialog"
    >
      <div className="dispatch-install-dialog">
        <div className="dispatch-install-dialog__header">
          <h2 id={DIALOG_TITLE_ID} className="dispatch-install-dialog__title">
            {t('dispatch.configureTitle', { target: target?.displayName ?? '' })}
          </h2>
          <span className="dispatch-install-dialog__subtitle">
            {t('dispatch.configureSubtitle')}
          </span>
        </div>

        <div className="dispatch-install-dialog__body">
          {error ? (
            <Alert type="error" message={error} closable onClose={() => setError(null)} />
          ) : null}
          {baseRefError ? (
            <Alert
              type="error"
              message={baseRefError}
              closable
              onClose={() => setBaseRefError(null)}
            />
          ) : null}

          <section className="dispatch-install-dialog__section">
            <div className="dispatch-install-dialog__section-header">
              <h3 className="dispatch-install-dialog__section-title">
                {t('dispatch.deliveryTitle')}
              </h3>
            </div>
            <div className="dispatch-install-dialog__section-body">
              <div className="dispatch-install-dialog__consent">
                <strong>{t('dispatch.baselineSource')}</strong>
                <code>{sourceWorkspacePath}</code>
                <span>{t('dispatch.baselineDescription')}</span>
                <label className="dispatch-install-dialog__base-ref">
                  <span>{t('dispatch.baseRef')}</span>
                  <input
                    type="text"
                    value={baseRef}
                    disabled={installing || validatingBaseRef}
                    spellCheck={false}
                    onChange={event => {
                      setBaseRef(event.target.value);
                      setBaseRefError(null);
                    }}
                    placeholder="HEAD"
                  />
                </label>
                <span className="dispatch-install-dialog__hint">
                  {t('dispatch.baseRefHint')}
                </span>
                <label>
                  <input
                    type="checkbox"
                    checked={includeUncommitted}
                    disabled={installing || validatingBaseRef}
                    onChange={event => {
                      includeUncommittedTouchedRef.current = true;
                      setIncludeUncommitted(event.target.checked);
                    }}
                  />
                  {t('dispatch.includeUncommitted')}
                </label>
                <span className="dispatch-install-dialog__hint">
                  {t('dispatch.includeUncommittedHint')}
                </span>
              </div>
            </div>
          </section>

          {probe ? (
            <section className="dispatch-install-dialog__section">
              <div className="dispatch-install-dialog__section-header">
                <h3 className="dispatch-install-dialog__section-title">
                  {t('dispatch.readinessTitle')}
                </h3>
              </div>
              <div className="dispatch-install-dialog__section-body">
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
                  <div data-state={modelReady ? 'ok' : 'blocked'}>
                    <span>{t('dispatch.modelStatus')}</span>
                    <strong>
                      {!modelReady
                        ? protocol?.modelDiagnostic || t('dispatch.modelMissing')
                        : modelParity === 'match'
                          ? t('dispatch.modelMatchesLocal', { model: targetDefaultModelLabel })
                          : modelParity === 'diverged'
                            ? t('dispatch.modelDiffersFromLocal', { count: targetModelCount })
                            : t('dispatch.modelReadyCount', { count: targetModelCount })}
                    </strong>
                  </div>
                </div>
              </div>
            </section>
          ) : null}

          {probe?.prebuiltIncompatible ? (
            <Alert type="warning" message={probe.prebuiltIncompatible} />
          ) : probe?.installError ? (
            <Alert type="warning" message={probe.installError} />
          ) : null}

          {target?.kind === 'ssh' && !cliReady && probe?.release ? (
            <section className="dispatch-install-dialog__section">
              <div className="dispatch-install-dialog__section-header">
                <h3 className="dispatch-install-dialog__section-title">
                  {t('dispatch.installAutomaticTitle')}
                </h3>
              </div>
              <div className="dispatch-install-dialog__section-body dispatch-install-dialog__action-panel">
                <span className="dispatch-install-dialog__hint">
                  {t('dispatch.installAutomaticDescription')}
                </span>
                {/* The digest is still shown: automatic installation removed the
                    prompt, not the verification it used to display. */}
                <dl>
                  <div><dt>{t('dispatch.version')}</dt><dd>{probe.release.version}</dd></div>
                  <div><dt>{t('dispatch.downloadUrl')}</dt><dd>{probe.release.url}</dd></div>
                  <div><dt>SHA256</dt><dd>{probe.release.sha256}</dd></div>
                </dl>
              </div>
            </section>
          ) : null}

          {target?.kind === 'ssh' && !cliReady && sourceBuild ? (
            <section className="dispatch-install-dialog__section">
              <div className="dispatch-install-dialog__section-header">
                <h3 className="dispatch-install-dialog__section-title">
                  {t('dispatch.sourceBuildTitle')}
                </h3>
              </div>
              <div className="dispatch-install-dialog__section-body dispatch-install-dialog__action-panel">
                <span className="dispatch-install-dialog__hint">
                  {t('dispatch.sourceBuildDescription', { ref: sourceBuild.gitRef })}
                </span>
                {sourceBuild.blockers.length > 0 ? (
                  <ul className="dispatch-install-dialog__blockers">
                    {sourceBuild.blockers.map(blocker => (
                      <li key={blocker}>{blocker}</li>
                    ))}
                  </ul>
                ) : null}
                <Button
                  variant="primary"
                  size="small"
                  disabled={installing || !sourceBuild.supported}
                  onClick={() => void startSourceBuild()}
                >
                  {installing ? <Loader2 size={14} className="dispatch-install-dialog__spin" /> : null}
                  {installing ? t('dispatch.installing') : t('dispatch.sourceBuildConfirm')}
                </Button>
              </div>
            </section>
          ) : null}

          {target?.kind === 'ssh' && probe?.protocol ? (
            <section className="dispatch-install-dialog__section">
              <div className="dispatch-install-dialog__section-header">
                <h3 className="dispatch-install-dialog__section-title">
                  {t('dispatch.syncModelRequired')}
                </h3>
              </div>
              <div className="dispatch-install-dialog__section-body dispatch-install-dialog__action-panel">
                <span className="dispatch-install-dialog__hint">
                  {t('dispatch.syncModelDescription')}
                </span>
                <Button
                  variant="primary"
                  size="small"
                  disabled={installing || syncingModel || probing}
                  onClick={() => void syncModelConfiguration()}
                >
                  {syncingModel ? <Loader2 size={14} className="dispatch-install-dialog__spin" /> : null}
                  {syncingModel ? t('dispatch.syncingModel') : t('dispatch.syncModelConfirm')}
                </Button>
              </div>
            </section>
          ) : null}

          {installStart || installOutput ? (
            <pre className="dispatch-install-dialog__output" aria-label={t('dispatch.installOutput')}>
              {installOutput || t('dispatch.installWaiting')}
            </pre>
          ) : null}

          <section className="dispatch-install-dialog__section">
            <div className="dispatch-install-dialog__section-header">
              <h3 className="dispatch-install-dialog__section-title">
                {t('dispatch.approvalTitle')}
              </h3>
            </div>
            <div className="dispatch-install-dialog__section-body">
              <span className="dispatch-install-dialog__hint">
                {t('dispatch.approvalHint')}
              </span>
              <fieldset
                className="dispatch-install-dialog__options"
                disabled={installing || validatingBaseRef}
              >
                <button
                  type="button"
                  role="radio"
                  className="dispatch-install-dialog__option"
                  aria-checked={approvalPolicy === 'reject-and-report'}
                  data-selected={approvalPolicy === 'reject-and-report'}
                  onClick={() => setApprovalPolicy('reject-and-report')}
                >
                  <ShieldAlert size={16} />
                  <span>
                    <strong>{t('dispatch.approvalReject')}</strong>
                    <small>{t('dispatch.approvalRejectDescription')}</small>
                  </span>
                  {approvalPolicy === 'reject-and-report' ? <Check size={16} /> : null}
                </button>
                <button
                  type="button"
                  role="radio"
                  className="dispatch-install-dialog__option"
                  aria-checked={approvalPolicy === 'remote'}
                  data-selected={approvalPolicy === 'remote'}
                  onClick={() => setApprovalPolicy('remote')}
                >
                  <ShieldQuestion size={16} />
                  <span>
                    <strong>{t('dispatch.approvalRemote')}</strong>
                    <small>{t('dispatch.approvalRemoteDescription')}</small>
                  </span>
                  {approvalPolicy === 'remote' ? <Check size={16} /> : null}
                </button>
                <button
                  type="button"
                  role="radio"
                  className="dispatch-install-dialog__option"
                  aria-checked={approvalPolicy === 'auto'}
                  data-selected={approvalPolicy === 'auto'}
                  onClick={() => setApprovalPolicy('auto')}
                >
                  <ShieldCheck size={16} />
                  <span>
                    <strong>{t('dispatch.approvalAuto')}</strong>
                    <small>{t('dispatch.approvalAutoDescription')}</small>
                  </span>
                  {approvalPolicy === 'auto' ? <Check size={16} /> : null}
                </button>
              </fieldset>
            </div>
          </section>
        </div>

        <div className="dispatch-install-dialog__actions">
          <Button variant="secondary" size="small" onClick={close}>
            {t('dispatch.cancel')}
          </Button>
          <Button
            variant="primary"
            size="small"
            disabled={
              !ready
              || installing
              || validatingBaseRef
              || worktreeSettingsLoading
            }
            onClick={() => void confirmTarget()}
          >
            {validatingBaseRef ? (
              <Loader2 size={14} className="dispatch-install-dialog__spin" />
            ) : null}
            {t('dispatch.useTarget')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};
