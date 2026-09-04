import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const modelSettingsSource = readFileSync(
  fileURLToPath(new URL('./ModelSettingsPage.tsx', import.meta.url)),
  'utf8',
);
const defaultModelSource = readFileSync(
  fileURLToPath(new URL('./DefaultModelConfig.tsx', import.meta.url)),
  'utf8',
);
const modelSettingsStyles = readFileSync(
  fileURLToPath(new URL('./ModelSettingsPage.scss', import.meta.url)),
  'utf8',
);

describe('model service collapsed presentation', () => {
  it('starts provider groups collapsed and exposes an accessible user toggle', () => {
    expect(modelSettingsSource).toContain(
      'const [expandedProviderGroupKeys, setExpandedProviderGroupKeys] = useState<Set<string>>(new Set());',
    );
    expect(modelSettingsSource).toContain('aria-expanded={isExpanded}');
    expect(modelSettingsSource).toContain("name={isExpanded ? 'chevron-down' : 'chevron-right'}");
    expect(modelSettingsSource).toContain('{isExpanded && (');
  });

  it('includes the leading card inset in the provider toggle hit area', () => {
    expect(modelSettingsStyles).toMatch(
      /&__provider-group-header\s*\{[\s\S]*?padding-inline:\s*0 var\(--bf-space-4\)/,
    );
    expect(modelSettingsStyles).toMatch(
      /&__provider-group-toggle\s*\{[\s\S]*?align-self:\s*stretch[\s\S]*?padding-inline-start:\s*var\(--bf-space-4\)/,
    );
  });

  it('marks the primary model slot as required in both label and control semantics', () => {
    expect(defaultModelSource).toMatch(
      /label=\{t\('core\.primary\.label'\)\}[\s\S]*?description=\{t\('core\.primary\.description'\)\}[\s\S]*?required[\s\S]*?<Combobox[\s\S]*?aria-required="true"/,
    );
  });

  it('keeps the enable switch at the trailing edge and reveals secondary model actions on interaction', () => {
    expect(modelSettingsSource).toMatch(
      /<span className="bitfun-model-settings__model-enable">[\s\S]*?<Switch[\s\S]*?<div[\s\S]*?className="bitfun-model-settings__model-actions"/,
    );
    expect(modelSettingsSource).toContain('data-bf-part="modelActions"');
    expect(modelSettingsSource).toContain('toggleOnRowClick');
  });

  it('uses the semantic highlight color for each provider model count', () => {
    expect(modelSettingsStyles).toMatch(
      /&__provider-group-count\s*\{[\s\S]*?color:\s*var\(--bf-color-content-required-indicator\)/,
    );
  });
});
