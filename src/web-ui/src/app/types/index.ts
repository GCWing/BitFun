/**
 * App feature type definitions.
 * Unified global layout across modes.
 */

import { i18nService } from '@/infrastructure/i18n';

// Agent types
export interface Agent {
  id: string;
  name: string;
  description: string;
  avatar?: string;
  capabilities: string[];
  isActive: boolean;
  config?: AgentConfig;
}

// Agent configuration
export interface AgentConfig {
  model: string;
  temperature: number;
  maxTokens: number;
  systemPrompt?: string;
  tools?: string[];
  extensions?: string[];
}

// Panel types - removed 'chat'; chat lives in the center panel.
// 'project-context' replaces 'context' to avoid overlap with agentic context.
export type PanelType = 'sessions' | 'workflows' | 'files' | 'git' | 'project-context' | 'terminal';

// Layout state - three-column layout support.
// Strategy: fixed left/right widths with elastic center (floating layout).
export interface LayoutState {
  leftPanelWidth: number;
  leftPanelCollapsed: boolean;
  centerPanelWidth: number;
  centerPanelCollapsed: boolean;
  chatCollapsed: boolean;
  rightPanelWidth: number; // Fixed right panel width
  rightPanelCollapsed: boolean;
  leftPanelActiveTab: PanelType;
  rightPanelTabs: TabInfo[];
  rightPanelActiveTabId: string | null;
}

// Tab info
export interface TabInfo {
  id: string;
  title: string;
  type: 'file' | 'diff' | 'extension' | 'chat' | 'preview';
  content?: any;
  isClosable: boolean;
  isDirty?: boolean;
  metadata?: Record<string, any>;
}

// Chat types
export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: Date;
  agentId?: string;
  metadata?: Record<string, any>;
}

export interface ChatSession {
  id: string;
  title: string;
  messages: ChatMessage[];
  agentId: string;
  createdAt: Date;
  updatedAt: Date;
  isActive: boolean;
}

// Extension types
export interface Extension {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  icon?: string;
  isEnabled: boolean;
  isInstalled: boolean;
  config?: Record<string, any>;
  capabilities: string[];
}

// App state
export interface AppState {
  layout: LayoutState;
  currentAgent: Agent | null;
  availableAgents: Agent[];
  chatSessions: ChatSession[];
  activeChatSession: ChatSession | null;
  extensions: Extension[];
  isLoading: boolean;
  error: string | null;
}

// App events
export type AppEvent = 
  | { type: 'layout:changed'; payload: Partial<LayoutState> }
  | { type: 'agent:changed'; payload: Agent }
  | { type: 'chat:message:sent'; payload: { sessionId: string; message: ChatMessage } }
  | { type: 'chat:message:received'; payload: { sessionId: string; message: ChatMessage } }
  | { type: 'chat:session:created'; payload: ChatSession }
  | { type: 'chat:session:selected'; payload: ChatSession }
  | { type: 'extension:enabled'; payload: Extension }
  | { type: 'extension:disabled'; payload: Extension }
  | { type: 'tab:opened'; payload: TabInfo }
  | { type: 'tab:closed'; payload: { tabId: string } }
  | { type: 'tab:selected'; payload: { tabId: string } }
  | { type: 'error:occurred'; payload: { error: string } };

// App manager interface
export interface IAppManager {
  // State management
  getState(): AppState;
  updateLayout(layout: Partial<LayoutState>): void;
  
  // Agent management - selectAgent removed in simplified flow
  updateAgentConfig(agentId: string, config: Partial<AgentConfig>): Promise<void>;
  
  // Chat management
  createChatSession(agentId: string): Promise<ChatSession>;
  selectChatSession(sessionId: string): void;
  sendMessage(sessionId: string, content: string): Promise<void>;
  
  // Extension management
  enableExtension(extensionId: string): Promise<void>;
  disableExtension(extensionId: string): Promise<void>;
  configureExtension(extensionId: string, config: Record<string, any>): Promise<void>;
  
  // Tab management
  openTab(tab: Omit<TabInfo, 'id'>): string;
  closeTab(tabId: string): void;
  selectTab(tabId: string): void;
  
  // Event listeners
  addEventListener(listener: (event: AppEvent) => void): () => void;
}

// Panel component props
export interface PanelProps {
  className?: string;
  isActive?: boolean;
  onActivate?: () => void;
}

export interface ChatPanelProps extends PanelProps {
  session: ChatSession | null;
  agent: Agent | null;
  onSendMessage: (content: string) => void;
  onCreateSession: (agentId: string) => void;
}


export interface FilesPanelProps extends PanelProps {
  workspacePath?: string;
  onFileSelect: (filePath: string) => void;
  onFileOpen: (filePath: string) => void;
}

// Bottom bar props
export interface BottomBarProps {
  activeTab: PanelType;
  onTabChange: (tab: PanelType) => void;
  onSendMessage?: (content: string) => void;
  showInputBox?: boolean;
  className?: string;
}

// Hook return type
export interface UseAppReturn {
  // State
  state: AppState;
  
  // Layout actions
  toggleLeftPanel: () => void;
  toggleCenterPanel: () => void;
  toggleRightPanel: () => void;
  toggleChatPanel: () => void;
  switchLeftPanelTab: (tab: PanelType) => void;
  updateLeftPanelWidth: (width: number, options?: { persist?: boolean }) => void;
  updateCenterPanelWidth: (width: number) => void;
  updateRightPanelWidth: (width: number) => void;
  
  // Agent actions - selectAgent removed; backend decides agent selection
  updateAgentConfig: (agentId: string, config: Partial<AgentConfig>) => Promise<void>;
  
  // Chat actions
  createChatSession: (agentId: string) => Promise<ChatSession>;
  selectChatSession: (sessionId: string) => void;
  sendMessage: (content: string) => Promise<void>;
  
  // Extension actions
  enableExtension: (extensionId: string) => Promise<void>;
  disableExtension: (extensionId: string) => Promise<void>;
  
  // Tab actions
  openTab: (tab: Omit<TabInfo, 'id'>) => string;
  closeTab: (tabId: string) => void;
  selectTab: (tabId: string) => void;
  
  // Utility
  clearError: () => void;
}

// Default layout state - three-column floating layout.
// Strategy: fixed left/right widths with elastic center.
// Left/right adjustments do not affect each other.
export const DEFAULT_LAYOUT_STATE: LayoutState = {
  leftPanelWidth: typeof window !== 'undefined'
    ? Math.min(400, Math.floor(window.innerWidth * 0.15)) // Left 15%, max 400px
    : 280,
  leftPanelCollapsed: false,
  centerPanelWidth: typeof window !== 'undefined' 
    ? Math.floor(window.innerWidth * 0.50) // Kept for compatibility
    : 960,
  centerPanelCollapsed: false,
  chatCollapsed: false,
  rightPanelWidth: typeof window !== 'undefined'
    ? Math.max(540, Math.min(800, Math.floor(window.innerWidth * 0.35))) // Right 35%, min 540px (for config-tabs), max 800px
    : 540,
  rightPanelCollapsed: true,
  leftPanelActiveTab: 'sessions', // Default to sessions list
  rightPanelTabs: [],
  rightPanelActiveTabId: null
};

// Default agents
export const DEFAULT_AGENTS: Agent[] = [
  {
    id: 'general',
    name: i18nService.t('common:agents.general.name'),
    description: i18nService.t('common:agents.general.description'),
    capabilities: ['chat', 'code', 'analysis'],
    isActive: true,
    config: {
      model: 'gpt-3.5-turbo',
      temperature: 0.7,
      maxTokens: 2048
    }
  },
  {
    id: 'coder',
    name: i18nService.t('common:agents.coder.name'),
    description: i18nService.t('common:agents.coder.description'),
    capabilities: ['code', 'debug', 'refactor'],
    isActive: false,
    config: {
      model: 'gpt-4',
      temperature: 0.3,
      maxTokens: 4096,
      tools: ['code_execution', 'file_operations']
    }
  }
];

// Message type enum
export enum MessageType {
  TEXT = 'text',
  CODE = 'code',
  FILE = 'file',
  IMAGE = 'image',
  ERROR = 'error',
  SYSTEM = 'system'
}

// Processing status
export enum ProcessingStatus {
  IDLE = 'idle',
  THINKING = 'thinking',
  PROCESSING = 'processing',
  STREAMING = 'streaming',
  COMPLETED = 'completed',
  ERROR = 'error'
}
