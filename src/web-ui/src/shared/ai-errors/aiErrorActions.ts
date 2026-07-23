export function openModelSettings(): void {
  void import('@/app/scenes/settings/settingsStore')
    .then(({ useSettingsStore }) => {
      useSettingsStore.getState().setActiveTab('models');
    })
    .catch(() => {
      // Opening the scene still gives the user a path to repair model settings.
    });

  window.dispatchEvent(new CustomEvent('scene:open', { detail: { sceneId: 'settings' } }));
}
