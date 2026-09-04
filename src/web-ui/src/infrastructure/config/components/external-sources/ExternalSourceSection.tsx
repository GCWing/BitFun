import React from 'react';
import { ConfigPageRow, ConfigPageSection } from '../common';
import {
  externalSourceDiagnosticKey,
  type ExternalSourcePresentationGroup,
} from '../../externalSourcePresentation';
import {
  SOURCE_COUNT_LABELS,
  isActionableSourceDiagnostic,
  sourceDiagnosticCategory,
  sourceScopeLabel,
} from './presentation';
import type { ExternalSectionCommonProps } from './types';

export interface ExternalSourceSectionProps
  extends Pick<ExternalSectionCommonProps, 't'> {
  groups: ExternalSourcePresentationGroup[];
  /** Renders the per-capability toggles owned by the controller. */
  renderSourceMembers: (group: ExternalSourcePresentationGroup) => React.ReactNode;
}

/**
 * Physical configuration sources grouped by presentation id. OpenCode is
 * aggregated separately, so this section renders the remaining ecosystems.
 */
export const ExternalSourceSection: React.FC<ExternalSourceSectionProps> = ({
  groups,
  t,
  renderSourceMembers,
}) => {
  if (groups.length === 0) return null;

  return (
    <ConfigPageSection title={t('sources.title')}>
      {groups.map((group) => {
        const userDiagnostics = group.diagnostics.filter(isActionableSourceDiagnostic);
        return (
          <React.Fragment key={group.key}>
            <ConfigPageRow
              className="openbitfun-external-sources-config__source-group"
              label={group.displayName}
              description={(
                <div className="openbitfun-external-sources-config__source-description" data-openbitfun-component="external-sources-config" data-openbitfun-part="sourceGroup">
                  <span className="openbitfun-external-sources-config__source-origin" data-openbitfun-component="external-sources-config" data-openbitfun-part="sourceDescription">
                    <span
                      className="openbitfun-external-sources-config__source-location"
                      title={group.location}
                      translate="no"
                    >
                      {group.location}
                    </span>
                    <span aria-hidden="true">·</span>
                    <span className="openbitfun-external-sources-config__source-scopes">
                      {group.scopes.map((scope, index) => (
                        <React.Fragment key={scope}>
                          {index > 0 ? <span aria-hidden="true"> + </span> : null}
                          <span>
                            {sourceScopeLabel(scope, t)}
                          </span>
                        </React.Fragment>
                      ))}
                    </span>
                  </span>
                  {SOURCE_COUNT_LABELS.some(
                    ([capability]) => group.counts[capability] > 0,
                  ) ? (
                    <span className="openbitfun-external-sources-config__source-counts">
                      {SOURCE_COUNT_LABELS.map(([capability, label]) => {
                        const count = group.counts[capability];
                        return count > 0 ? (
                          <span
                            key={capability}
                            className="openbitfun-external-sources-config__source-count"
                          >
                            {t(label, { count })}
                          </span>
                        ) : null;
                      })}
                    </span>
                  ) : null}
                </div>
              )}
              align="center"
            >
              {renderSourceMembers(group)}
            </ConfigPageRow>
            {userDiagnostics.length > 0 ? (
              <details
                className="openbitfun-external-sources-config__notice"
                data-openbitfun-component="external-sources-config"
                data-openbitfun-part="notice"
                data-external-attention="true"
                data-external-ecosystem={group.ecosystemId}
              >
                <summary>
                  {t('diagnostics.sourceSummary', {
                    name: group.displayName,
                    count: userDiagnostics.length,
                  })}
                </summary>
                <ul className="openbitfun-external-sources-config__diagnostics" data-openbitfun-component="external-sources-config" data-openbitfun-part="diagnostics">
                  {userDiagnostics.map((diagnostic) => (
                    <li key={externalSourceDiagnosticKey(diagnostic)}>
                      <span>{t(`diagnostics.category.${sourceDiagnosticCategory(diagnostic.code)}`)}</span>
                    </li>
                  ))}
                </ul>
              </details>
            ) : null}
          </React.Fragment>
        );
      })}
    </ConfigPageSection>
  );
};
