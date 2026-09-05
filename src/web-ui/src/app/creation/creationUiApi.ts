import { productControlAPI } from '@/infrastructure/api/service-api/ProductControlAPI';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { isPeerDeviceModeActive } from '@/infrastructure/peer-device/peerModeFlag';
import { createCreationCapabilities } from '@/infrastructure/creation/creationCapabilities';
import type { ProductControlCapabilityId, ProductControlOperationId, ProductControlOptionId } from '@/infrastructure/api/generated/productControl';
import { useSceneStore } from '../stores/sceneStore';

export const CREATION_PARTS = Object.freeze({
  shell: '[data-openbitfun-component="app-layout"][data-openbitfun-part="root"]',
  workspace: '[data-openbitfun-scene="workbench"][data-openbitfun-part="workspace"]',
  sidebar: '[data-openbitfun-scene="workbench"][data-openbitfun-part="navArea"]',
  content: '[data-openbitfun-scene="workbench"][data-openbitfun-part="sceneArea"]',
  tabs: '[data-openbitfun-component="scene-bar"][data-openbitfun-part="root"]',
});
export const CREATION_SLOTS = ['sidebar-footer', 'scene-header', 'scene-footer'] as const;
type CreationSlot = typeof CREATION_SLOTS[number];

/** The shell owns the stores; custom code receives a small supported facade. */
export function createCreationUiApi(signal: AbortSignal) {
  const cleanups = new Set<() => void>();
  let disposed = false;
  const assertActive = () => {
    if (disposed) throw new Error('Creation runtime has been deactivated');
    signal.throwIfAborted();
    if (isPeerDeviceModeActive()) throw new Error('UI customization is unavailable on a Peer Device surface');
  };
  const capabilities = createCreationCapabilities({ assertActive, storage: window.localStorage });
  const inspect = () => {
    assertActive();
    return {
      ...capabilities.inspect(),
      scene: useSceneStore.getState().activeTabId,
      parts: Object.fromEntries(Object.entries(CREATION_PARTS).map(([name, selector]) =>
        [name, { selector, present: document.querySelector(selector) !== null }])),
      slots: CREATION_SLOTS.map(id => {
        const host = document.querySelector(`[data-openbitfun-creation-slot="${id}"]`);
        return { id, present: host !== null, mounts: host?.childElementCount ?? 0 };
      }),
    };
  };
  cleanups.add(useSceneStore.subscribe((state, previous) => {
    if (!signal.aborted && !disposed && !isPeerDeviceModeActive() && state.activeTabId !== previous.activeTabId) {
      void capabilities.events.emit('scene.changed', { scene: state.activeTabId });
    }
  }));
  const api = Object.freeze({
    version: 1 as const,
    signal,
    parts: CREATION_PARTS,
    inspect,
    commands: capabilities.commands,
    state: capabilities.state,
    events: capabilities.events,
    mount(slot: CreationSlot): HTMLDivElement {
      assertActive();
      if (!CREATION_SLOTS.includes(slot)) throw new Error(`Unknown UI customization slot: ${slot}`);
      const host = document.querySelector(`[data-openbitfun-creation-slot="${slot}"]`);
      if (!host) throw new Error(`UI customization slot is unavailable: ${slot}`);
      const root = document.createElement('div');
      host.append(root);
      cleanups.add(() => root.remove());
      return root;
    },
    getScene() { assertActive(); return useSceneStore.getState().activeTabId; },
    onSceneChange(listener: (id: string | null) => void) {
      assertActive();
      const unsubscribe = useSceneStore.subscribe((state, previous) => {
        if (!signal.aborted && !isPeerDeviceModeActive() && state.activeTabId !== previous.activeTabId) listener(state.activeTabId);
      });
      cleanups.add(unsubscribe);
      return unsubscribe;
    },
    async openMiniApp(appId: string) {
      assertActive();
      await miniAppAPI.getMiniApp(appId);
      assertActive();
      useSceneStore.getState().openScene(`miniapp:${appId}`);
    },
    control: Object.freeze({
      get(id: ProductControlCapabilityId) { assertActive(); return productControlAPI.get(id); },
      open(id: ProductControlCapabilityId, itemId?: string) { assertActive(); return productControlAPI.open(id, itemId); },
      configure<C extends ProductControlCapabilityId>(id: C, option: ProductControlOptionId<C>, value: unknown) {
        assertActive(); return productControlAPI.configure(id, option, value);
      },
      execute<C extends ProductControlCapabilityId>(id: C, operation: ProductControlOperationId<C>, args?: Record<string, unknown>) {
        assertActive(); return productControlAPI.execute(id, operation, args);
      },
    }),
  });
  return {
    api,
    inspect,
    invoke: capabilities.commands.invoke,
    dispose() {
      if (disposed) return;
      disposed = true;
      capabilities.dispose();
      for (const cleanup of cleanups) cleanup();
      cleanups.clear();
    },
  };
}
