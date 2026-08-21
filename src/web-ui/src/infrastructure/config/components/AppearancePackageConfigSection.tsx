import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, Download, Image, Store, Trash2, Upload, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Select, Tooltip, confirmDialog } from '@/component-library';
import {
  SYSTEM_APPEARANCE_ID,
  getAppearancePackageValidationError,
  useAppearance,
  type AppearancePackageValidationError,
  type AppearanceValidationIssue,
} from '@/infrastructure/appearance';
import { notificationService } from '@/shared/notification-system';
import { AppearanceMarketDialog } from './AppearanceMarketDialog';

function downloadArchive(bytes: ArrayBuffer, filename: string): void {
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

export type AppearancePackageFailure = {
  operation: 'import' | 'activate';
  validationError?: AppearancePackageValidationError;
  message?: string;
};

function issueText(
  issue: AppearanceValidationIssue,
  t: ReturnType<typeof useTranslation>['t'],
): string {
  if (issue.code === 'UNKNOWN_PART' && issue.context?.partId) {
    return t('package.diagnostics.unknownPart', { part: issue.context.partId });
  }
  if (issue.code === 'UNKNOWN_SURFACE') {
    return t(issue.context?.surfaceKind === 'scene'
      ? 'package.diagnostics.unknownScene'
      : 'package.diagnostics.unknownComponent');
  }
  return issue.message;
}

export function AppearancePackageFailurePanel({
  failure,
  onDismiss,
}: {
  failure: AppearancePackageFailure;
  onDismiss: () => void;
}) {
  const { t } = useTranslation('settings/appearance');
  const validationError = failure.validationError;
  const title = validationError
    ? t('package.diagnostics.validationTitle')
    : t(failure.operation === 'import'
      ? 'package.diagnostics.importTitle'
      : 'package.diagnostics.activateTitle');

  return (
    <section
      className="appearance-package-config__diagnostics"
      role="alert"
      aria-live="polite"
      data-bf-component="appearance-settings"
      data-bf-part="packageDiagnostics"
    >
      <div
        className="appearance-package-config__diagnostics-header"
        data-bf-component="appearance-settings"
        data-bf-part="packageDiagnosticsHeader"
      >
        <AlertTriangle size={17} aria-hidden="true" />
        <div>
          <strong>{title}</strong>
          {validationError && (
            <p>{t('package.diagnostics.validationHint', { count: validationError.issues.length })}</p>
          )}
        </div>
        <Button
          variant="ghost"
          size="small"
          iconOnly
          title={t('package.diagnostics.dismiss')}
          aria-label={t('package.diagnostics.dismiss')}
          onClick={onDismiss}
        >
          <X size={14} />
        </Button>
      </div>

      {validationError ? (
        <div className="appearance-package-config__diagnostics-groups">
          {validationError.groups.map(group => (
            <section
              key={group.key}
              className="appearance-package-config__diagnostics-group"
              data-bf-component="appearance-settings"
              data-bf-part="packageDiagnosticsGroup"
            >
              <h4>
                {group.surfaceKind === 'component'
                  ? t('package.diagnostics.componentGroup', { id: group.surfaceId })
                  : group.surfaceKind === 'scene'
                    ? t('package.diagnostics.sceneGroup', { id: group.surfaceId })
                    : t('package.diagnostics.sectionGroup', { id: group.section })}
              </h4>
              <ul>
                {group.issues.map(issue => (
                  <li
                    key={`${issue.code}:${issue.path}`}
                    data-bf-component="appearance-settings"
                    data-bf-part="packageDiagnosticIssue"
                  >
                    <span>{issueText(issue, t)}</span>
                    <code>{issue.path}</code>
                  </li>
                ))}
              </ul>
              {group.allowedParts.length > 0 && (
                <details
                  className="appearance-package-config__diagnostics-parts"
                  data-bf-component="appearance-settings"
                  data-bf-part="packageDiagnosticAllowedParts"
                >
                  <summary>{t('package.diagnostics.allowedParts')}</summary>
                  <div>{group.allowedParts.map(part => <code key={part}>{part}</code>)}</div>
                </details>
              )}
            </section>
          ))}
        </div>
      ) : (
        <p className="appearance-package-config__diagnostics-message">{failure.message}</p>
      )}
    </section>
  );
}

function AppearancePackagePreview({
  appearanceId,
  appearanceName,
  getPreviewAsset,
}: {
  appearanceId: string;
  appearanceName: string;
  getPreviewAsset: ReturnType<typeof useAppearance>['getPreviewAsset'];
}) {
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    let objectUrl: string | null = null;
    setPreviewUrl(null);
    void getPreviewAsset(appearanceId)
      .then(asset => {
        if (!asset || disposed) return;
        objectUrl = URL.createObjectURL(new Blob([asset.bytes], { type: asset.mimeType }));
        setPreviewUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [appearanceId, getPreviewAsset]);

  return (
    <Tooltip
      placement="top"
      delay={180}
      content={(
        <div
          className="appearance-package-config__preview-popover"
          data-testid="appearance-package-preview-popover"
        >
          <div className="appearance-package-config__preview-popover-image">
            {previewUrl
              ? <img src={previewUrl} alt="" />
              : <Image size={40} aria-hidden="true" />}
          </div>
          <span>{appearanceName}</span>
        </div>
      )}
    >
      <span
        className="appearance-package-config__preview"
        role="img"
        aria-label={appearanceName}
        data-testid="appearance-package-preview-thumbnail"
        data-bf-component="appearance-settings"
        data-bf-part="packagePreview"
      >
        {previewUrl
          ? <img src={previewUrl} alt="" />
          : <Image size={15} aria-hidden="true" />}
      </span>
    </Tooltip>
  );
}

export function AppearancePackageConfigSection() {
  const { t } = useTranslation('settings/appearance');
  const inputRef = useRef<HTMLInputElement>(null);
  const [loading, setLoading] = useState(false);
  const [marketOpen, setMarketOpen] = useState(false);
  const [failure, setFailure] = useState<AppearancePackageFailure | null>(null);
  const {
    appearances: appearanceCatalog,
    unavailableSelectionId,
    selectedAppearanceId,
    getPreviewAsset,
    importPackage,
    exportPackage,
    activate,
    deletePackage,
    status,
  } = useAppearance();
  const appearances = useMemo(
    () => appearanceCatalog.filter(appearance => appearance.source === 'imported'),
    [appearanceCatalog],
  );
  const selectedAppearance = appearances.find(appearance => appearance.id === selectedAppearanceId);
  const selectedPackageId = selectedAppearance?.id ?? SYSTEM_APPEARANCE_ID;
  const selectedPackageName = selectedAppearance?.name ?? t('package.nativeName');
  const packageOptions = useMemo(() => [
    {
      value: SYSTEM_APPEARANCE_ID,
      label: t('package.nativeName'),
      description: t('package.nativeDescription'),
      testId: 'appearance-package-option',
      testAttributes: { 'data-appearance-id': SYSTEM_APPEARANCE_ID },
    },
    ...appearances.map(appearance => ({
      value: appearance.id,
      label: appearance.name,
      description: `${appearance.author || t('package.unknownAuthor')} · v${appearance.version}`,
      testId: 'appearance-package-option',
      testAttributes: { 'data-appearance-id': appearance.id },
    })),
  ], [appearances, t]);
  const busy = loading || status === 'applying';

  const handleImport = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;
    setLoading(true);
    try {
      await importPackage(await file.arrayBuffer());
      setFailure(null);
      notificationService.success(t('package.importSuccess', { name: file.name }));
    } catch (error) {
      const validationError = getAppearancePackageValidationError(error);
      setFailure({
        operation: 'import',
        ...(validationError
          ? { validationError }
          : { message: error instanceof Error ? error.message : String(error) }),
      });
      notificationService.error(validationError
        ? t('package.diagnostics.importSummary', { count: validationError.issues.length })
        : t('package.importFailed'), { duration: 5000 });
    } finally {
      setLoading(false);
    }
  };

  const handleExport = async (id: string) => {
    try {
      downloadArchive(await exportPackage(id), `${id}.bitfun-appearance`);
    } catch (error) {
      notificationService.error(t('package.exportFailed', {
        error: error instanceof Error ? error.message : String(error),
      }));
    }
  };

  const handleActivate = async (id: string | null) => {
    setLoading(true);
    try {
      if (id) await activate(id);
      else await activate(SYSTEM_APPEARANCE_ID);
      setFailure(null);
    } catch (error) {
      const validationError = getAppearancePackageValidationError(error);
      setFailure({
        operation: 'activate',
        ...(validationError
          ? { validationError }
          : { message: error instanceof Error ? error.message : String(error) }),
      });
      notificationService.error(validationError
        ? t('package.diagnostics.activateSummary', { count: validationError.issues.length })
        : t('package.activateFailed'), { duration: 5000 });
    } finally {
      setLoading(false);
    }
  };

  const handleSelectionChange = (value: string | number | (string | number)[]) => {
    if (Array.isArray(value)) return;
    const nextId = String(value);
    void handleActivate(nextId === SYSTEM_APPEARANCE_ID ? null : nextId);
  };

  const handleDelete = async (id: string, name: string) => {
    const confirmed = await confirmDialog({
      title: t('package.deleteTitle'),
      message: t('package.deleteMessage', { name }),
      confirmText: t('package.delete'),
      confirmDanger: true,
      type: 'warning',
    });
    if (!confirmed) return;
    setLoading(true);
    try {
      await deletePackage(id);
      notificationService.success(t('package.deleteSuccess', { name }));
    } catch (error) {
      notificationService.error(t('package.deleteFailed', {
        error: error instanceof Error ? error.message : String(error),
      }));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="appearance-package-config"
      role="group"
      aria-label={t('package.title')}
      data-bf-component="appearance-settings"
      data-bf-part="packageSection"
      data-bf-package-type={selectedAppearance ? 'imported' : 'native'}
      data-bf-state={busy ? 'disabled' : undefined}
    >
      <input
        ref={inputRef}
        className="appearance-package-config__file-input"
        type="file"
        accept=".bitfun-appearance,.zip,application/zip"
        onChange={handleImport}
      />
      <div className="appearance-package-config__inline-control">
        <AppearancePackagePreview
          appearanceId={selectedPackageId}
          appearanceName={selectedPackageName}
          getPreviewAsset={getPreviewAsset}
        />
        <div
          className="appearance-package-config__select-control"
          data-bf-component="appearance-settings"
          data-bf-part="packageSelect"
        >
          <Select
            size="small"
            value={selectedPackageId}
            options={packageOptions}
            onChange={handleSelectionChange}
            disabled={busy}
            placement="bottom"
            triggerTestId="appearance-package-select"
          />
        </div>
        <div
          className="appearance-package-config__actions"
          data-bf-component="appearance-settings"
          data-bf-part="packageActions"
        >
          <Button
            variant="ghost"
            size="small"
            iconOnly
            title={t('package.market.open')}
            aria-label={t('package.market.open')}
            disabled={busy}
            onClick={() => setMarketOpen(true)}
          >
            <Store size={14} />
          </Button>
          <Button
            variant="ghost"
            size="small"
            iconOnly
            title={t('package.import')}
            aria-label={t('package.import')}
            disabled={busy}
            onClick={() => inputRef.current?.click()}
          >
            <Upload size={14} />
          </Button>
          {selectedAppearance && (
            <>
              <Button
                variant="ghost"
                size="small"
                iconOnly
                title={t('package.export')}
                aria-label={t('package.export')}
                disabled={busy}
                onClick={() => void handleExport(selectedAppearance.id)}
              >
                <Download size={14} />
              </Button>
              <Button
                variant="ghost"
                size="small"
                iconOnly
                title={t('package.delete')}
                aria-label={t('package.delete')}
                disabled={busy}
                onClick={() => void handleDelete(selectedAppearance.id, selectedAppearance.name)}
              >
                <Trash2 size={14} />
              </Button>
            </>
          )}
        </div>
      </div>
      <AppearanceMarketDialog isOpen={marketOpen} onClose={() => setMarketOpen(false)} />
      {failure && (
        <AppearancePackageFailurePanel failure={failure} onDismiss={() => setFailure(null)} />
      )}
      {unavailableSelectionId && (
        <div
          className="appearance-package-config__missing-selection"
          data-bf-component="appearance-settings"
          data-bf-part="packageMissingSelection"
        >
          <AlertTriangle size={16} aria-hidden="true" />
          <span>{t('package.missingSelection', { id: unavailableSelectionId })}</span>
          <Button variant="secondary" size="small" onClick={() => setMarketOpen(true)}>
            {t('package.market.open')}
          </Button>
        </div>
      )}
    </div>
  );
}
