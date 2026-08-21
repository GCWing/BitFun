import { create } from 'zustand';
import { useSceneStore } from '@/app/stores/sceneStore';
import type { EcosystemProductId } from './ecosystemCompatibilityModel';

export type EcosystemCompatibilityOwnerSurface = 'acp' | 'external-sources' | null;

export interface EcosystemCompatibilityDestination {
  productId?: EcosystemProductId;
  ownerSurface?: Exclude<EcosystemCompatibilityOwnerSurface, null>;
}

interface EcosystemCompatibilityState {
  selectedProductId: EcosystemProductId;
  ownerSurface: EcosystemCompatibilityOwnerSurface;
  selectProduct: (productId: EcosystemProductId) => void;
  setOwnerSurface: (ownerSurface: EcosystemCompatibilityOwnerSurface) => void;
  openDestination: (destination: EcosystemCompatibilityDestination) => void;
}

export const useEcosystemCompatibilityStore = create<EcosystemCompatibilityState>((set) => ({
  selectedProductId: 'codex',
  ownerSurface: null,
  selectProduct: (selectedProductId) => set({
    selectedProductId,
    ownerSurface: null,
  }),
  setOwnerSurface: (ownerSurface) => set({ ownerSurface }),
  openDestination: (destination) => set((state) => ({
    selectedProductId: destination.productId ?? state.selectedProductId,
    ownerSurface: destination.ownerSurface ?? state.ownerSurface,
  })),
}));

/** Opens the product surface and optionally selects one of its real owner views. */
export function openEcosystemCompatibility(
  destination?: EcosystemCompatibilityDestination,
): void {
  if (destination) {
    useEcosystemCompatibilityStore.getState().openDestination(destination);
  }
  useSceneStore.getState().openScene('ecosystem-compatibility');
}
