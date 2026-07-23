/**
 * Mode state reducer
 */

export interface ModeInfo {
  id: string;
  name: string;
  description: string;
  isReadonly: boolean;
  toolCount: number;
  defaultTools?: string[];
  /**
   * Combined prompt-cache compatibility key returned by the backend.
   * Modes with the same key can reuse the same session-level prompt cache.
   */
  promptCacheScopeKey: string;
}

export interface ModeState {
  /** Current mode id */
  current: string;
  /** Available modes */
  available: ModeInfo[];
  /** Dropdown open state */
  dropdownOpen: boolean;
}

export type ModeAction =
  | { type: 'SET_CURRENT_MODE'; payload: string }
  | { type: 'SET_AVAILABLE_MODES'; payload: ModeInfo[] }
  | { type: 'OPEN_DROPDOWN' }
  | { type: 'CLOSE_DROPDOWN' }
  | { type: 'TOGGLE_DROPDOWN' };

export const initialModeState: ModeState = {
  current: 'agentic',
  available: [],
  dropdownOpen: false,
};

export function modeReducer(state: ModeState, action: ModeAction): ModeState {
  switch (action.type) {
    case 'SET_CURRENT_MODE':
      return { ...state, current: action.payload };
      
    case 'SET_AVAILABLE_MODES':
      return { ...state, available: action.payload };
      
    case 'OPEN_DROPDOWN':
      return { ...state, dropdownOpen: true };
      
    case 'CLOSE_DROPDOWN':
      return { ...state, dropdownOpen: false };
      
    case 'TOGGLE_DROPDOWN':
      return { ...state, dropdownOpen: !state.dropdownOpen };
      
    default:
      return state;
  }
}

