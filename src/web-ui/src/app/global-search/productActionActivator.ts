import { useNavSceneStore } from '@/app/stores/navSceneStore';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import type { ProductActionId } from './productActionCatalog';

export interface ProductActionActivationOptions {
  behavior?: 'open' | 'toggle';
  t?: (key: string, options?: Record<string, unknown>) => string;
}

export async function activateProductAction(
  actionId: ProductActionId,
  options: ProductActionActivationOptions = {},
): Promise<void> {
  const sceneStore = useSceneStore.getState();

  switch (actionId) {
    case 'session.new':
      window.dispatchEvent(new CustomEvent('toolbar-create-session'));
      return;
    case 'project.open': {
      const { pickWorkspaceDirectory } = await import(
        '@/infrastructure/peer-device/pickWorkspaceDirectory'
      );
      const selected = await pickWorkspaceDirectory({
        title: options.t?.('header.selectProjectDirectory') ?? 'Open project',
      });
      if (selected) await workspaceManager.openWorkspace(selected);
      return;
    }
    case 'project.new':
      window.dispatchEvent(new Event('nav:new-project'));
      return;
    case 'surface.browser.open':
      if (sceneStore.activeTabId === 'session') {
        window.dispatchEvent(new CustomEvent('agent-create-tab', {
          detail: {
            type: 'browser',
            title: options.t?.('scenes.browser') ?? 'Browser',
            checkDuplicate: true,
            duplicateCheckKey: 'browser-panel',
            replaceExisting: false,
          },
        }));
      } else {
        sceneStore.openScene('browser');
      }
      return;
    case 'surface.terminal.open': {
      const navStore = useNavSceneStore.getState();
      if (
        options.behavior === 'toggle'
        && navStore.showSceneNav
        && navStore.navSceneId === 'shell'
      ) {
        navStore.closeNavScene();
      } else {
        navStore.openNavScene('shell');
      }
      return;
    }
    case 'surface.files.open':
      sceneStore.openScene('file-viewer');
      return;
    case 'surface.agents.open':
      sceneStore.openScene('agents');
      return;
    case 'surface.skills.open':
      sceneStore.openScene('skills');
      return;
    case 'surface.miniapps.open':
      sceneStore.openScene('miniapps');
      return;
    case 'surface.todos.open':
      sceneStore.openScene('todos');
      return;
    case 'surface.insights.open':
      sceneStore.openScene('insights');
      return;
    case 'settings.open':
      sceneStore.openScene('settings');
      return;
    case 'settings.keyboard.open':
      useSettingsStore.getState().openTab('keyboard');
      sceneStore.openScene('settings');
      return;
    case 'settings.external-sources.open':
      useSettingsStore.getState().openTab('external-sources');
      sceneStore.openScene('settings');
      return;
  }
}
