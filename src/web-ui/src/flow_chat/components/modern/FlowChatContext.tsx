/**
 * FlowChat context.
 * Pass callbacks and config through the tree to avoid prop drilling.
 */

import { createContext, useContext } from 'react';
import type { FlowChatConfig, Session } from '../../types/flow-chat';
import type { LineRange } from '@/component-library';

export interface FlowChatContextValue {
  // File and panel actions
  onFileViewRequest?: (filePath: string, fileName: string, lineRange?: LineRange) => void;
  onTabOpen?: (tabInfo: any, sessionId?: string, panelType?: string) => void;
  onOpenVisualization?: (type: string, data: any) => void;
  onSwitchToChatPanel?: () => void;

  // Tool actions
  onToolConfirm?: (toolId: string, updatedInput?: any) => Promise<void>;
  onToolReject?: (toolId: string) => Promise<void>;

  // Session info
  sessionId?: string;
  activeSessionOverride?: Session | null;

  // Config
  config?: FlowChatConfig;

  // ========== Explore group collapse state ==========
  /**
   * Expanded/collapsed state for explore groups.
   * key: groupId, value: true means expanded.
   */
  exploreGroupStates?: Map<string, boolean>;

  /**
   * Toggle explore group expanded/collapsed state.
   */
  onExploreGroupToggle?: (groupId: string) => void;

  /**
   * Expand the specified explore group.
   */
  onExpandGroup?: (groupId: string) => void;

  /**
   * Expand all explore groups within a turn.
   */
  onExpandAllInTurn?: (turnId: string) => void;

  /**
   * Collapse the specified explore group.
   */
  onCollapseGroup?: (groupId: string) => void;

  // ========== Message search state ==========
  /** Active search query string. Empty string means no active search. */
  searchQuery?: string;
  /** Set of virtual item indices that contain search matches. */
  searchMatchIndices?: ReadonlySet<number>;
  /** Virtual item index of the currently focused search match, or -1 if none. */
  searchCurrentMatchVirtualIndex?: number;
}

export const FlowChatContext = createContext<FlowChatContextValue>({});

/**
 * FlowChat context hook.
 */
export const useFlowChatContext = () => {
  return useContext(FlowChatContext);
};


