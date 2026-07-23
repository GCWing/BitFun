import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { AlertTriangle, CircleDashed, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Tooltip } from '@/component-library';
import { ExternalSourceApiError } from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import {
  externalHooksAPI,
  type ExternalHookCatalogEntry,
  type ExternalHookCatalogSnapshot,
  type ExternalHookDiagnostic,
  type ExternalHookProviderIdentity,
  type ExternalHookSource,
} from '@/infrastructure/api/service-api/ExternalHooksAPI';
import { createLogger } from '@/shared/utils/logger';
import { ConfigPageSection } from './common';
import './ExternalHooksPanel.scss';

const logger = createLogger('ExternalHooksPanel');
const INITIAL_SOURCE_LIMIT = 20;
const SOURCE_PAGE_SIZE = 20;
const INITIAL_ENTRY_LIMIT = 100;
const ENTRY_PAGE_SIZE = 100;
const INITIAL_DIAGNOSTIC_LIMIT = 20;
const DIAGNOSTIC_PAGE_SIZE = 20;

export interface ExternalHooksPanelProps {
  workspacePath?: string;
  unsupportedReason?: 'remote' | 'peer';
  refreshEpoch?: number;
}

function sourceKey(source: ExternalHookSource['key']): string {
  return `${source.providerId}\u0000${source.sourceId}`;
}

function sourceScopeLabel(source: ExternalHookSource, t: TFunction): string {
  return source.scope === 'workspace_local'
    ? t('shared:features.workspace')
    : t(`hooks.scope.${source.scope}`);
}

function matcherLabel(entry: ExternalHookCatalogEntry, t: TFunction): string {
  switch (entry.matcher.kind) {
    case 'any': return t('hooks.matcherValue.all');
    case 'pattern': return entry.matcher.display;
    case 'dynamic': return t('hooks.matcherValue.dynamic');
    case 'unavailable': return t('hooks.matcherValue.unavailable');
    default: return t('hooks.matcherValue.unknown');
  }
}

function projectionLabel(entry: ExternalHookCatalogEntry, t: TFunction): string {
  if (entry.projectionStatus === 'opaque') return t('hooks.projection.opaque');
  if (!entry.mapping) return t('hooks.projection.nativeOnly');
  return t(`hooks.projection.${entry.mapping.hookPoint}`);
}

function diagnosticCategory(code: string): string {
  if (code.endsWith('all_disabled')) return 'nativeDisabled';
  if (code.endsWith('activation_not_evaluated')) return 'activationUnknown';
  if (code.endsWith('coverage_static_only') || code.endsWith('package_declared_only')) {
    return 'staticCoverage';
  }
  if (code.endsWith('registration_opaque')) return 'opaque';
  if (code.includes('limit') || code.endsWith('too_large')) return 'inspectionLimit';
  if (code.includes('parse') || code.includes('invalid')) return 'invalidConfiguration';
  if (code.includes('unreadable') || code.includes('failed') || code.includes('unavailable')) {
    return 'unavailable';
  }
  return 'generic';
}

function HookDiagnosticItem({ diagnostic, t }: {
  diagnostic: ExternalHookDiagnostic;
  t: TFunction;
}) {
  return (
    <li>
      <span>{t(`hooks.diagnosticCategory.${diagnosticCategory(diagnostic.code)}`, {
        code: diagnostic.code,
      })}</span>
      <details className="bitfun-external-hooks__technical-detail">
        <summary>{t('hooks.technicalDetails')}</summary>
        <code>{diagnostic.code}</code>
        <span>{diagnostic.message}</span>
      </details>
    </li>
  );
}

function HookSourceCard({
  source,
  entries,
  diagnostics,
  totalEntries,
  stale,
  t,
}: {
  source: ExternalHookSource;
  entries: ExternalHookCatalogEntry[];
  diagnostics: ExternalHookDiagnostic[];
  totalEntries: number;
  stale: boolean;
  t: TFunction;
}) {
  return (
    <div className="bitfun-external-hooks__source">
      <div className="bitfun-external-hooks__source-heading">
        <div>
          <div className="bitfun-external-hooks__source-name">{source.displayName}</div>
          <div className="bitfun-external-hooks__location">
            {source.locationHint}
            {' · '}
            {sourceScopeLabel(source, t)}
          </div>
        </div>
        <span className={`bitfun-external-hooks__health is-${source.health}`}>
          {t(stale ? 'hooks.health.stale' : `hooks.health.${source.health}`)}
        </span>
      </div>
      {totalEntries === 0 ? (
        <div className="bitfun-external-hooks__empty-source">{t('hooks.noEntries')}</div>
      ) : entries.length === 0 ? (
        <div className="bitfun-external-hooks__empty-source">{t('hooks.entriesDeferred')}</div>
      ) : (
        <ul className="bitfun-external-hooks__entries">
          {entries.map((entry) => (
            <li key={entry.stableKey}>
              <div className="bitfun-external-hooks__entry-main">
                <code>{entry.nativeEvent}</code>
                <span>{t(`hooks.handler.${entry.handlerKind}`)}</span>
              </div>
              <div className="bitfun-external-hooks__entry-detail">
                <span>{t('hooks.matcher', { matcher: matcherLabel(entry, t) })}</span>
                <span>{projectionLabel(entry, t)}</span>
                <span>{t(`hooks.activation.${entry.nativeActivation}`)}</span>
              </div>
            </li>
          ))}
        </ul>
      )}
      {diagnostics.length > 0 ? (
        <ul className="bitfun-external-hooks__diagnostics">
          {diagnostics.map((diagnostic) => (
            <HookDiagnosticItem
              key={`${diagnostic.code}:${diagnostic.message}`}
              diagnostic={diagnostic}
              t={t}
            />
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function HookProviderSection({
  provider,
  sources,
  entriesBySource,
  staleProviderIds,
  failedProviderIds,
  discoveryPending,
  initiallyOpen,
  t,
}: {
  provider: ExternalHookProviderIdentity;
  sources: ExternalHookSource[];
  entriesBySource: Map<string, ExternalHookCatalogEntry[]>;
  staleProviderIds: string[];
  failedProviderIds: string[];
  discoveryPending: boolean;
  initiallyOpen: boolean;
  t: TFunction;
}) {
  const [open, setOpen] = useState(initiallyOpen);
  const [sourceLimit, setSourceLimit] = useState(INITIAL_SOURCE_LIMIT);
  const [entryLimit, setEntryLimit] = useState(INITIAL_ENTRY_LIMIT);
  const [diagnosticLimit, setDiagnosticLimit] = useState(INITIAL_DIAGNOSTIC_LIMIT);
  useEffect(() => {
    if (initiallyOpen) setOpen(true);
  }, [initiallyOpen]);
  const entries = sources.flatMap((source) => entriesBySource.get(sourceKey(source.key)) ?? []);
  const mappedCount = entries.filter((entry) => entry.projectionStatus === 'mapped').length;
  const visibleSources = sources.slice(0, sourceLimit);
  let remainingEntryBudget = entryLimit;
  let remainingDiagnosticBudget = diagnosticLimit;
  const renderedSources = visibleSources.map((source) => {
    const sourceEntries = entriesBySource.get(sourceKey(source.key)) ?? [];
    const renderedEntries = sourceEntries.slice(0, remainingEntryBudget);
    const renderedDiagnostics = source.diagnostics.slice(0, remainingDiagnosticBudget);
    remainingEntryBudget -= renderedEntries.length;
    remainingDiagnosticBudget -= renderedDiagnostics.length;
    return {
      source,
      entries: renderedEntries,
      diagnostics: renderedDiagnostics,
      totalEntries: sourceEntries.length,
    };
  });
  const visibleEntryCount = visibleSources.reduce(
    (count, source) => count + (entriesBySource.get(sourceKey(source.key))?.length ?? 0),
    0,
  );
  const renderedEntryCount = renderedSources.reduce((count, item) => count + item.entries.length, 0);
  const visibleDiagnosticCount = visibleSources.reduce(
    (count, source) => count + source.diagnostics.length,
    0,
  );
  const renderedDiagnosticCount = renderedSources.reduce(
    (count, item) => count + item.diagnostics.length,
    0,
  );
  const stale = staleProviderIds.includes(provider.providerId);
  const failed = failedProviderIds.includes(provider.providerId);
  return (
    <details
      className="bitfun-external-hooks__ecosystem"
      open={open}
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary>
        <span>{provider.displayName}</span>
        <span className="bitfun-external-hooks__counts">
          {t('hooks.summary', { hooks: entries.length, mapped: mappedCount, sources: sources.length })}
          {failed
            ? ` · ${t('hooks.health.failed')}`
            : stale ? ` · ${t('hooks.health.stale')}` : null}
        </span>
      </summary>
      {open ? (
        <div className="bitfun-external-hooks__sources">
          {sources.length === 0 && !discoveryPending ? (
            <div className="bitfun-external-hooks__empty-source">
              {t(failed ? 'hooks.providerFailed' : stale ? 'hooks.providerStaleEmpty' : 'hooks.providerEmpty')}
            </div>
          ) : null}
          {renderedSources.map(({
            source,
            entries: renderedEntries,
            diagnostics: renderedDiagnostics,
            totalEntries,
          }) => (
            <HookSourceCard
              key={sourceKey(source.key)}
              source={source}
              entries={renderedEntries}
              diagnostics={renderedDiagnostics}
              totalEntries={totalEntries}
              stale={stale}
              t={t}
            />
          ))}
          {visibleSources.length < sources.length ? (
            <Button
              variant="ghost"
              size="small"
              aria-label={t('hooks.showMoreSourcesForProvider', {
                count: sources.length - visibleSources.length,
                provider: provider.displayName,
              })}
              onClick={() => setSourceLimit((value) => value + SOURCE_PAGE_SIZE)}
            >
              {t('hooks.showMoreSources', { count: sources.length - visibleSources.length })}
            </Button>
          ) : null}
          {renderedEntryCount < visibleEntryCount ? (
            <Button
              variant="ghost"
              size="small"
              aria-label={t('hooks.showMoreEntriesForProvider', {
                count: visibleEntryCount - renderedEntryCount,
                provider: provider.displayName,
              })}
              onClick={() => setEntryLimit((value) => value + ENTRY_PAGE_SIZE)}
            >
              {t('hooks.showMoreEntries', { count: visibleEntryCount - renderedEntryCount })}
            </Button>
          ) : null}
          {renderedDiagnosticCount < visibleDiagnosticCount ? (
            <Button
              variant="ghost"
              size="small"
              aria-label={t('hooks.showMoreDiagnosticsForProvider', {
                count: visibleDiagnosticCount - renderedDiagnosticCount,
                provider: provider.displayName,
              })}
              onClick={() => setDiagnosticLimit((value) => value + DIAGNOSTIC_PAGE_SIZE)}
            >
              {t('hooks.showMoreDiagnostics', {
                count: visibleDiagnosticCount - renderedDiagnosticCount,
              })}
            </Button>
          ) : null}
        </div>
      ) : null}
    </details>
  );
}

const ExternalHooksPanel: React.FC<ExternalHooksPanelProps> = ({
  workspacePath,
  unsupportedReason,
  refreshEpoch = 0,
}) => {
  const { t } = useTranslation(['settings/external-sources', 'shared']);
  const [snapshot, setSnapshot] = useState<ExternalHookCatalogSnapshot | null>(null);
  const [loading, setLoading] = useState(!unsupportedReason);
  const [refreshing, setRefreshing] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [catalogDiagnosticLimit, setCatalogDiagnosticLimit] = useState(INITIAL_DIAGNOSTIC_LIMIT);
  const requestSequence = useRef(0);

  const loadCatalog = useCallback(async (forceRefresh: boolean, showLoading = true) => {
    if (unsupportedReason) return null;
    const sequence = ++requestSequence.current;
    if (forceRefresh) setRefreshing(true);
    if (showLoading) setLoading(true);
    try {
      const next = await externalHooksAPI.getCatalog(workspacePath, forceRefresh);
      if (sequence !== requestSequence.current) return null;
      setSnapshot(next);
      setErrorCode(null);
      return next;
    } catch (error) {
      if (sequence !== requestSequence.current) return null;
      const code = error instanceof ExternalSourceApiError ? error.code : 'internal';
      logger.warn('Failed to load external Hook catalog', { code });
      setErrorCode(code);
      return null;
    } finally {
      if (sequence === requestSequence.current) {
        if (showLoading) setLoading(false);
        setRefreshing(false);
      }
    }
  }, [unsupportedReason, workspacePath]);

  useEffect(() => {
    requestSequence.current += 1;
    setSnapshot(null);
    setErrorCode(null);
    setCatalogDiagnosticLimit(INITIAL_DIAGNOSTIC_LIMIT);
    if (unsupportedReason) {
      setLoading(false);
      setRefreshing(false);
      return;
    }
    void loadCatalog(true);
    // `refreshEpoch` deliberately joins initial/workspace and header refreshes.
  }, [loadCatalog, refreshEpoch, unsupportedReason]);

  useEffect(() => {
    if (unsupportedReason || loading || errorCode || !snapshot?.discoveryPending) return undefined;
    let cancelled = false;
    let timer: number | undefined;
    const poll = async () => {
      const next = await loadCatalog(false, false);
      if (!cancelled && next?.discoveryPending) {
        timer = window.setTimeout(() => void poll(), 250);
      }
    };
    timer = window.setTimeout(() => void poll(), 250);
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [errorCode, loadCatalog, loading, snapshot?.discoveryPending, unsupportedReason]);

  const providers = useMemo(() => (snapshot?.providers ?? []).map((provider) => ({
    provider,
    sources: snapshot?.sources.filter((source) => source.key.providerId === provider.providerId) ?? [],
  })), [snapshot]);

  const entriesBySource = useMemo(() => {
    const grouped = new Map<string, ExternalHookCatalogEntry[]>();
    for (const entry of snapshot?.entries ?? []) {
      const key = sourceKey(entry.source);
      const entries = grouped.get(key) ?? [];
      entries.push(entry);
      grouped.set(key, entries);
    }
    return grouped;
  }, [snapshot]);

  const catalogDiagnostics = useMemo(
    () => snapshot?.diagnostics.filter((diagnostic) => diagnostic.source === undefined) ?? [],
    [snapshot],
  );
  const visibleCatalogDiagnostics = catalogDiagnostics.slice(0, catalogDiagnosticLimit);
  const busy = loading
    || refreshing
    || (Boolean(snapshot?.discoveryPending) && !errorCode);
  const firstProviderWithSources = providers.findIndex((group) => group.sources.length > 0);
  const refreshLabel = t(refreshing ? 'hooks.refreshing' : 'hooks.refresh');
  const refresh = unsupportedReason ? undefined : (
    <Tooltip content={refreshLabel} placement="top">
      <Button
        variant="ghost"
        size="small"
        aria-label={refreshLabel}
        disabled={busy}
        onClick={() => void loadCatalog(true)}
      >
        <RefreshCw
          className={refreshing ? 'bitfun-external-hooks__spinner' : undefined}
          size={15}
          aria-hidden="true"
        />
      </Button>
    </Tooltip>
  );

  return (
    <ConfigPageSection
      className="bitfun-external-hooks"
      title={t('hooks.title')}
      description={t('hooks.description')}
      extra={refresh}
    >
      <div className="bitfun-external-hooks__body" aria-busy={busy}>
        {unsupportedReason ? (
          <div className="bitfun-external-hooks__state" role="status">
            <AlertTriangle size={16} aria-hidden="true" />
            <span>{t(`hooks.unsupported.${unsupportedReason}`)}</span>
          </div>
        ) : loading && !snapshot ? (
          <div className="bitfun-external-hooks__state" role="status" aria-live="polite">
            <CircleDashed className="bitfun-external-hooks__spinner" size={16} aria-hidden="true" />
            <span>{t('hooks.loading')}</span>
          </div>
        ) : errorCode && !snapshot ? (
          <div className="bitfun-external-hooks__state" role="alert">
            <AlertTriangle size={16} aria-hidden="true" />
            <span>{t('hooks.error', { code: errorCode })}</span>
          </div>
        ) : (
          <div className="bitfun-external-hooks__ecosystems">
            {errorCode ? (
              <div className="bitfun-external-hooks__notice" role="status">
                {t(
                  snapshot?.discoveryPending
                    ? 'hooks.pollInterrupted'
                    : 'hooks.staleAfterRefresh',
                  { code: errorCode },
                )}
              </div>
            ) : null}
            {refreshing ? (
              <div className="bitfun-external-hooks__notice" role="status" aria-live="polite">
                {t('hooks.refreshing')}
              </div>
            ) : null}
            {snapshot?.discoveryPending && !errorCode ? (
              <div className="bitfun-external-hooks__notice" role="status" aria-live="polite">
                {t('hooks.pending')}
              </div>
            ) : null}
            {snapshot
            && !snapshot.discoveryPending
            && snapshot.sources.length === 0
            && (snapshot.failedProviderIds ?? []).length === 0
            && snapshot.staleProviderIds.length === 0 ? (
              <div className="bitfun-external-hooks__state" role="status" aria-live="polite">
                {t('hooks.empty')}
              </div>
            ) : null}
            {providers.map(({ provider, sources }, index) => (
              <HookProviderSection
                key={provider.providerId}
                provider={provider}
                sources={sources}
                entriesBySource={entriesBySource}
                staleProviderIds={snapshot?.staleProviderIds ?? []}
                failedProviderIds={snapshot?.failedProviderIds ?? []}
                discoveryPending={snapshot?.discoveryPending ?? false}
                initiallyOpen={index === (firstProviderWithSources >= 0 ? firstProviderWithSources : 0)}
                t={t}
              />
            ))}
            {catalogDiagnostics.length > 0 ? (
              <div className="bitfun-external-hooks__catalog-diagnostics">
                <div>{t('hooks.diagnostics')}</div>
                <ul>
                  {visibleCatalogDiagnostics.map((diagnostic) => (
                    <HookDiagnosticItem
                      key={`${diagnostic.code}:${diagnostic.message}`}
                      diagnostic={diagnostic}
                      t={t}
                    />
                  ))}
                </ul>
                {visibleCatalogDiagnostics.length < catalogDiagnostics.length ? (
                  <Button
                    variant="ghost"
                    size="small"
                    onClick={() => setCatalogDiagnosticLimit(
                      (value) => value + DIAGNOSTIC_PAGE_SIZE,
                    )}
                  >
                    {t('hooks.showMoreDiagnostics', {
                      count: catalogDiagnostics.length - visibleCatalogDiagnostics.length,
                    })}
                  </Button>
                ) : null}
              </div>
            ) : null}
            <div className="bitfun-external-hooks__footnote">{t('hooks.readOnly')}</div>
          </div>
        )}
      </div>
    </ConfigPageSection>
  );
};

export default ExternalHooksPanel;
