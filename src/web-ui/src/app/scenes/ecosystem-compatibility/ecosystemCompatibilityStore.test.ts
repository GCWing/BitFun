import { beforeEach, describe, expect, it } from 'vitest';
import { useEcosystemCompatibilityStore } from './ecosystemCompatibilityStore';

describe('ecosystemCompatibilityStore', () => {
  beforeEach(() => {
    useEcosystemCompatibilityStore.setState({
      selectedProductId: 'codex',
      ownerSurface: null,
    });
  });

  it('opens a real owner surface without copying its state', () => {
    useEcosystemCompatibilityStore.getState().openDestination({
      ownerSurface: 'external-sources',
    });

    expect(useEcosystemCompatibilityStore.getState()).toMatchObject({
      selectedProductId: 'codex',
      ownerSurface: 'external-sources',
    });
  });

  it('returns to the product overview when another product is selected', () => {
    useEcosystemCompatibilityStore.setState({ ownerSurface: 'external-sources' });

    useEcosystemCompatibilityStore.getState().selectProduct('opencode');

    expect(useEcosystemCompatibilityStore.getState()).toMatchObject({
      selectedProductId: 'opencode',
      ownerSurface: null,
    });
  });
});
