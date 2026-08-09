export type { ExternalHostCapabilities, ExternalSectionCommonProps } from './types';
export {
  SOURCE_COUNT_LABELS,
  abbreviatedLocation,
  sourceDiagnosticCategory,
  sourceScopeLabel,
} from './presentation';
export { ExternalCommandConflicts } from './ExternalCommandConflicts';
export type { ExternalCommandConflictsProps } from './ExternalCommandConflicts';
export { ExternalSourceSection } from './ExternalSourceSection';
export type { ExternalSourceSectionProps } from './ExternalSourceSection';
export { useExternalAppAwareness } from './useExternalAppAwareness';
export { ExternalAppsOverview } from './ExternalAppsOverview';
export type { ExternalAppsOverviewProps } from './ExternalAppsOverview';
export {
  buildExternalApplicationsView,
  buildExternalApplicationsViewV2,
} from './applicationModel';
export type {
  ExternalApplicationView,
  ExternalApplicationsView,
  ExternalApplicationStatus,
  ExternalApplicationAction,
} from './applicationModel';
