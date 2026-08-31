import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const appearanceSettingsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'appearance-settings',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'settings' },
    { id: 'settingsContent' }, { id: 'language' }, { id: 'palettePicker' },
    { id: 'paletteSelect' },
    { id: 'packageSection' }, { id: 'packageSelect' }, { id: 'packagePreview' },
    { id: 'packageActions' },
    { id: 'packageDiagnostics' }, { id: 'packageDiagnosticsHeader' },
    { id: 'packageDiagnosticsGroup' }, { id: 'packageDiagnosticIssue' },
    { id: 'packageDiagnosticAllowedParts' },
    { id: 'packageMissingSelection' },
    { id: 'marketDialog' }, { id: 'marketToolbar' }, { id: 'marketBrowse' },
    { id: 'marketResults' }, { id: 'marketGrid' },
    { id: 'marketCard' }, { id: 'marketPreview' }, { id: 'marketCardBody' },
    { id: 'marketStatus' }, { id: 'marketEmpty' }, { id: 'marketError' },
    { id: 'marketDetail' }, { id: 'marketDetailPreview' }, { id: 'marketDetailBody' },
    { id: 'marketWarning' }, { id: 'marketReleaseList' }, { id: 'marketRelease' },
    { id: 'marketActions' }, { id: 'marketNav' }, { id: 'marketWorkflow' },
    { id: 'marketManualSubmit' },
    { id: 'marketSubmissionList' }, { id: 'marketSubmission' },
    { id: 'marketReviewLayout' }, { id: 'marketReviewQueue' },
    { id: 'marketReviewDetail' }, { id: 'marketReviewActions' },
  ],
  facets: [
    { id: 'packageType', attribute: 'data-bf-package-type', values: ['native', 'imported'] },
  ],
  states: [
    { id: 'hover', selector: { kind: 'self', suffix: ':hover' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'disabled', selector: { kind: 'self', suffix: '[data-bf-state~="disabled"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
  ],
};
