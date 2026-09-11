/**
 * Type definitions for panel components.
 * Defines content types, interfaces, and configuration for the panel system.
 */

import type { IconSource } from '@openbitfun/ui';

import type { PanelContent, PanelContentType } from '@/shared/types/panelContent';
export type { PanelContent, PanelContentType } from '@/shared/types/panelContent';

export interface TabData {
  id: string;
  title: string;
  content: PanelContent;
  isDirty?: boolean; // Has unsaved changes
}

export interface FlexiblePanelProps {
  content: PanelContent | null;
  onContentChange?: (content: PanelContent | null) => void;
  className?: string;
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  workspacePath?: string;
  onBeforeClose?: (content: PanelContent | null) => Promise<boolean>;
  terminalResizeSuspended?: boolean;
}

export interface TabbedFlexiblePanelProps {
  className?: string;
  onTabsChange?: (tabs: TabData[]) => void;
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  workspacePath?: string;
  onBeforeClose?: (content: PanelContent | null) => Promise<boolean>;
}

export interface TabbedFlexiblePanelRef {
  addTab: (content: PanelContent) => void;
  switchToTab: (tabId: string) => void;
  findTabByMetadata: (metadata: Record<string, any>) => TabData | null;
  updateTabContent: (tabId: string, content: PanelContent) => void;
  closeAllTabs: () => void;
}

export interface PanelContentConfig {
  type: PanelContentType;
  displayName: string;
  icon: IconSource;
  supportsCopy: boolean;
  supportsDownload: boolean;
  showHeader: boolean;
}
