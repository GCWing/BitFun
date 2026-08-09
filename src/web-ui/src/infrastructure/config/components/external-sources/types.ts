import type { TFunction } from 'i18next';
import type React from 'react';
import type { ExternalSourceCatalogSnapshot } from '@/infrastructure/api/service-api/ExternalSourcesAPI';

/**
 * Host capability gates travel with the snapshot, so section components read
 * them from the same authoritative object the controller validated.
 */
export type ExternalHostCapabilities = ExternalSourceCatalogSnapshot['hostCapabilities'];

/**
 * Shared contract for every external-sources section.
 *
 * Sections are presentation-only: they hold no state, issue no requests, and
 * derive no policy. The controller owns request sequencing, mutation fencing
 * and stale-response rejection, and passes results down through these props.
 */
export interface ExternalSectionCommonProps {
  snapshot: ExternalSourceCatalogSnapshot;
  t: TFunction;
  /** Mutation key currently in flight, used to drive per-control loading state. */
  busyKey: string | null;
  hostCapabilities: ExternalHostCapabilities;
  policyCompatible: boolean;
  /** Renders a reveal-in-explorer link, or a disabled hint when unsupported. */
  renderPathLink: (location: string, sourceKey?: string) => React.ReactNode;
}
