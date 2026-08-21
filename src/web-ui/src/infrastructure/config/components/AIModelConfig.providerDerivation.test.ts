// @vitest-environment node
//
// P2-2 regression: provider search/filter derivations must stay scoped to the
// provider-selection branch. The main model list should not recompute them on
// every render. This is a source-shape assertion on a 3700+ line config page:
// the component has no isolated DOM test bed, and the shape contract (top-level
// state, branch-scoped derivation) is exactly what P2-2 fixes.

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const source = readFileSync(fileURLToPath(new URL('./AIModelConfig.tsx', import.meta.url)), 'utf8');

describe('AIModelConfig provider selection derivation scope (P2-2)', () => {
  it('keeps providerQuery/showAllProviders as top-level state hooks', () => {
    expect(source).toMatch(/const \[providerQuery, setProviderQuery\] = useState\(''\);/);
    expect(source).toMatch(/const \[showAllProviders, setShowAllProviders\] = useState\(false\);/);
  });

  it('does not derive matchedProviders at the component top level', () => {
    // The derived values must only exist inside the creationMode === 'selection'
    // branch, so the top-level section (before the first conditional return)
    // must not declare them.
    const selectionBranchStart = source.indexOf("if (creationMode === 'selection')");
    expect(selectionBranchStart).toBeGreaterThan(0);
    const topLevel = source.slice(0, selectionBranchStart);
    expect(topLevel).not.toContain('matchedProviders');
    expect(topLevel).not.toContain('visibleProviders');
    expect(topLevel).not.toContain('canToggleProviderList');
    expect(topLevel).not.toContain('isProviderListCollapsed');
  });

  it('computes matchedProviders/visibleProviders inside the selection branch', () => {
    const selectionBranchStart = source.indexOf("if (creationMode === 'selection')");
    const selectionBranch = source.slice(selectionBranchStart);
    expect(selectionBranch).toContain('const normalizedProviderQuery = providerQuery.trim().toLowerCase();');
    expect(selectionBranch).toContain('const matchedProviders = normalizedProviderQuery');
    expect(selectionBranch).toContain('const canToggleProviderList = !normalizedProviderQuery');
    expect(selectionBranch).toContain('const visibleProviders = isProviderListCollapsed');
  });
});
