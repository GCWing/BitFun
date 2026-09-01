/**
 * Input state reducer
 */

export interface InputState {
  value: string;
}

export type InputAction =
  | { type: 'SET_VALUE'; payload: string }
  | { type: 'CLEAR_VALUE' };

export const initialInputState: InputState = {
  value: '',
};

export function inputReducer(state: InputState, action: InputAction): InputState {
  switch (action.type) {
    case 'SET_VALUE':
      return { ...state, value: action.payload };
      
    case 'CLEAR_VALUE':
      return { ...state, value: '' };

    default:
      return state;
  }
}
