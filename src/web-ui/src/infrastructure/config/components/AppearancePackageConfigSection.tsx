import React, { useEffect, useRef, useState } from 'react';
import { AlertTriangle, Check, Download, Image, Store, Trash2, Upload, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, confirmDialog } from '@/component-library';
import {
  SYSTEM_APPEARANCE_ID,
  getAppearancePackageValidationError,
  useAppearance,
  type AppearancePackageValidationError,
  type AppearanceValidationIssue,
} from '@/infrastructure/appearance';
import { notificationService } from '@/shared/notification-system';
import { AppearanceMarketDialog } from './AppearanceMarketDialog';
import { ConfigPageSection } from './common';

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
      data-bf-component="appearance-config"
      data-bf-part="packageDiagnostics"
    >
      <div
        className="appearance-package-config__diagnostics-header"
        data-bf-component="appearance-config"
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
              data-bf-component="appearance-config"
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
                    data-bf-component="appearance-config"
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
                  data-bf-component="appearance-config"
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
    <div
      className="appearance-package-config__preview"
      data-bf-component="appearance-config"
      data-bf-part="packagePreview"
    >
      {previewUrl
        ? <img src={previewUrl} alt={appearanceName} />
        : <Image size={22} aria-hidden="true" />}
    </div>
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
  const appearances = appearanceCatalog.filter(appearance => appearance.source === 'imported');
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
    <ConfigPageSection
      className="appearance-package-config"
      title={t('package.title')}
      description={t('package.description')}
      data-bf-component="appearance-config"
      data-bf-part="packageSection"
      extra={(
        <div className="appearance-package-config__header-actions">
          <input
            ref={inputRef}
            className="appearance-package-config__file-input"
            type="file"
            accept=".bitfun-appearance,.zip,application/zip"
            onChange={handleImport}
          />
          <Button variant="secondary" size="small" disabled={busy} onClick={() => setMarketOpen(true)}>
            <Store size={14} />
            {t('package.market.open')}
          </Button>
          <Button variant="secondary" size="small" disabled={busy} onClick={() => inputRef.current?.click()}>
            <Upload size={14} />
            {t('package.import')}
          </Button>
        </div>
      )}
    >
      <AppearanceMarketDialog isOpen={marketOpen} onClose={() => setMarketOpen(false)} />
      {failure && (
        <AppearancePackageFailurePanel failure={failure} onDismiss={() => setFailure(null)} />
      )}
      {unavailableSelectionId && (
        <div
          className="appearance-package-config__missing-selection"
          data-bf-component="appearance-config"
          data-bf-part="packageMissingSelection"
        >
          <AlertTriangle size={16} aria-hidden="true" />
          <span>{t('package.missingSelection', { id: unavailableSelectionId })}</span>
          <Button variant="secondary" size="small" onClick={() => setMarketOpen(true)}>
            {t('package.market.open')}
          </Button>
        </div>
      )}
      <div
        className="appearance-package-config__grid"
        data-bf-component="appearance-config"
        data-bf-part="packageGrid"
      >
        <button
          type="button"
          className={`appearance-package-config__card${selectedAppearanceId === SYSTEM_APPEARANCE_ID ? ' is-active' : ''}`}
          aria-pressed={selectedAppearanceId === SYSTEM_APPEARANCE_ID}
          disabled={busy}
          onClick={() => void handleActivate(null)}
          data-bf-component="appearance-config"
          data-bf-part="packageCard"
          data-bf-package-type="native"
          data-bf-state={[
            selectedAppearanceId === SYSTEM_APPEARANCE_ID && 'selected',
            busy && 'disabled',
          ].filter(Boolean).join(' ') || undefined}
        >
          <div
            className="appearance-package-config__native-preview"
            data-bf-component="appearance-config"
            data-bf-part="packagePreview"
          ><Image size={22} /></div>
          <div
            className="appearance-package-config__card-body"
            data-bf-component="appearance-config"
            data-bf-part="packageCardBody"
          >
            <strong>{t('package.nativeName')}</strong>
            <span>{t('package.nativeDescription')}</span>
          </div>
          {selectedAppearanceId === SYSTEM_APPEARANCE_ID && (
            <Check
              className="appearance-package-config__active"
              size={16}
              data-bf-component="appearance-config"
              data-bf-part="packageActiveIndicator"
            />
          )}
        </button>

        {appearances.map(appearance => {
          const active = selectedAppearanceId === appearance.id;
          return (
            <article
              key={appearance.id}
              className={`appearance-package-config__card${active ? ' is-active' : ''}`}
              data-bf-component="appearance-config"
              data-bf-part="packageCard"
              data-bf-package-type="imported"
              data-bf-state={[active && 'selected', busy && 'disabled'].filter(Boolean).join(' ') || undefined}
            >
              <button
                type="button"
                className="appearance-package-config__select"
                aria-pressed={active}
                disabled={busy}
                onClick={() => void handleActivate(appearance.id)}
                data-bf-component="appearance-config"
                data-bf-part="packageSelect"
              >
                <AppearancePackagePreview
                  appearanceId={appearance.id}
                  appearanceName={appearance.name}
                  getPreviewAsset={getPreviewAsset}
                />
                <div
                  className="appearance-package-config__card-body"
                  data-bf-component="appearance-config"
                  data-bf-part="packageCardBody"
                >
                  <strong>{appearance.name}</strong>
                  <span>{appearance.author || t('package.unknownAuthor')} · v{appearance.version}</span>
                </div>
              </button>
              {active && (
                <Check
                  className="appearance-package-config__active"
                  size={16}
                  data-bf-component="appearance-config"
                  data-bf-part="packageActiveIndicator"
                />
              )}
              <div
                className="appearance-package-config__actions"
                data-bf-component="appearance-config"
                data-bf-part="packageActions"
              >
                <Button variant="ghost" size="small" iconOnly title={t('package.export')} aria-label={t('package.export')} disabled={busy} onClick={() => void handleExport(appearance.id)}>
                  <Download size={14} />
                </Button>
                <Button variant="ghost" size="small" iconOnly title={t('package.delete')} aria-label={t('package.delete')} disabled={busy} onClick={() => void handleDelete(appearance.id, appearance.name)}>
                  <Trash2 size={14} />
                </Button>
              </div>
            </article>
          );
        })}
      </div>
      {appearances.length === 0 && (
        <p
          className="appearance-package-config__empty"
          data-bf-component="appearance-config"
          data-bf-part="packageEmpty"
        >
          {t('package.empty')}
        </p>
      )}
    </ConfigPageSection>
  );
}
