import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { useSceneStore } from '@/app/stores/sceneStore';
import { getInteractiveCapability } from './interactiveCapabilityCatalog';
import { activateProductAction } from './productActionActivator';

export interface InteractiveCapabilityActivationOptions {
  t?: (key: string, options?: Record<string, unknown>) => string;
}

export async function activateInteractiveCapability(
  capabilityId: string,
  options: InteractiveCapabilityActivationOptions = {},
): Promise<void> {
  const capability = getInteractiveCapability(capabilityId);
  if (!capability) throw new Error(`Unknown BitFun capability: ${capabilityId}`);

  switch (capability.destination.kind) {
    case 'settings':
      useSettingsStore.getState().openDestination(capability.destination);
      useSceneStore.getState().openScene('settings');
      return;
    case 'action':
      await activateProductAction(capability.destination.actionId, { t: options.t });
      return;
    case 'scene':
      useSceneStore.getState().openScene(capability.destination.sceneId);
      return;
    case 'event':
      window.dispatchEvent(new CustomEvent(capability.destination.eventName, {
        detail: capability.destination.detail,
      }));
  }
}
