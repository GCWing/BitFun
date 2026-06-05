export interface DeferredStartupGateState {
  interactiveShellReady: boolean;
  startupOverlayVisible: boolean;
}

export function shouldScheduleDeferredStartupSystems(state: DeferredStartupGateState): boolean {
  return state.interactiveShellReady && !state.startupOverlayVisible;
}
