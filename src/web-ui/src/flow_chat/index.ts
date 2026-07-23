/**
 * FlowChat Module Exports
 */

export { ModernFlowChatContainer as FlowChatContainer } from './components/modern/ModernFlowChatContainer';

// Other components
export { ChatInput } from './components/ChatInput';
export { CurrentSessionTitle } from './components/CurrentSessionTitle';
export { ScrollToBottomButton } from './components/ScrollToBottomButton';
export { ScrollToLatestBar } from './components/ScrollToLatestBar';

// Toolbar Mode components
export { 
  ToolbarMode,
  ToolbarModeProvider,
  useToolbarModeContext,
  type ToolbarModeProps,
  type ToolbarModeState
} from './components/toolbar-mode';

// Services and Stores
export { FlowChatManager } from './services/FlowChatManager';

// State machine
export { stateMachineManager } from './state-machine';
export type { TodoItem } from './state-machine/types';
