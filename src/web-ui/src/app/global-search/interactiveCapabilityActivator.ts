import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { useSceneStore } from '@/app/stores/sceneStore';
import { getInteractiveCapability } from './interactiveCapabilityCatalog';
import { activateProductAction } from './productActionActivator';

export interface InteractiveCapabilityActivationOptions {
  t?: (key: string, options?: Record<string, unknown>) => string;
  itemId?: string;
}

export async function activateInteractiveCapability(
  capabilityId: string,
  options: InteractiveCapabilityActivationOptions = {},
): Promise<void> {
  const capability = getInteractiveCapability(capabilityId);
  if (!capability) throw new Error(`Unknown BitFun capability: ${capabilityId}`);
  const item = options.itemId
    ? capability.items.find(({ id }) => id === options.itemId)
    : undefined;
  if (options.itemId && !item) {
    throw new Error(`Unknown documented item for ${capabilityId}: ${options.itemId}`);
  }
  const destination = item?.destination ?? capability.destination;

  switch (destination.kind) {
    case 'settings':
      useSettingsStore.getState().openDestination(destination);
      useSceneStore.getState().openScene('settings');
      return;
    case 'action':
      await activateProductAction(destination.actionId, { t: options.t });
      return;
    case 'scene':
      useSceneStore.getState().openScene(destination.sceneId);
      return;
    case 'event':
      window.dispatchEvent(new CustomEvent(destination.eventName, {
        detail: destination.detail,
      }));
  }
}
