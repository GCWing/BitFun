/**
 * AuxPane — AI Agent scene right pane.
 * Hosts ContentCanvas with tab management for editor views and visualizations.
 *
 * Renamed from panels/ContentPanel. All logic preserved.
 */

import { forwardRef, useEffect, useRef, useImperativeHandle, useCallback } from 'react';
import { ContentCanvas, useCanvasStore } from '../../components/panels/content-canvas';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import type { PanelContent as OldPanelContent } from '../../components/panels/base/types';
import type { PanelContent } from '../../components/panels/content-canvas/types';
import { createLogger } from '@/shared/utils/logger';

import './AuxPane.scss';

const log = createLogger('AuxPane');

export interface AuxPaneRef {
  addTab: (content: OldPanelContent) => void;
  switchToTab: (tabId: string) => void;
  findTabByMetadata: (metadata: Record<string, any>) => { tabId: string } | null;
  updateTabContent: (tabId: string, content: OldPanelContent) => void;
  closeAllTabs: () => void;
}

interface AuxPaneProps {
  workspacePath?: string;
}

const AuxPane = forwardRef<AuxPaneRef, AuxPaneProps>(
  ({ workspacePath }, ref) => {
    const {
      addTab,
      switchToTab,
      findTabByMetadata,
      updateTabContent,
      closeAllTabs,
      primaryGroup,
      secondaryGroup,
    } = useCanvasStore();

    const convertContent = useCallback((oldContent: OldPanelContent): PanelContent => {
      return {
        type: oldContent.type,
        title: oldContent.title,
        data: oldContent.data,
        metadata: oldContent.metadata,
      };
    }, []);

    useImperativeHandle(ref, () => ({
      addTab: (content: OldPanelContent) => {
        addTab(convertContent(content), 'active');
        window.dispatchEvent(new CustomEvent('expand-right-panel'));
      },
      switchToTab: (tabId: string) => {
        if (primaryGroup.tabs.find(t => t.id === tabId)) {
          switchToTab(tabId, 'primary');
          window.dispatchEvent(new CustomEvent('expand-right-panel'));
        } else if (secondaryGroup.tabs.find(t => t.id === tabId)) {
          switchToTab(tabId, 'secondary');
          window.dispatchEvent(new CustomEvent('expand-right-panel'));
        }
      },
      findTabByMetadata: (metadata: Record<string, any>) => {
        const result = findTabByMetadata(metadata);
        return result ? { tabId: result.tab.id } : null;
      },
      updateTabContent: (tabId: string, content: OldPanelContent) => {
        if (primaryGroup.tabs.find(t => t.id === tabId)) {
          updateTabContent(tabId, 'primary', convertContent(content));
        } else if (secondaryGroup.tabs.find(t => t.id === tabId)) {
          updateTabContent(tabId, 'secondary', convertContent(content));
        }
      },
      closeAllTabs: () => {
        closeAllTabs();
      },
    }), [
      addTab,
      switchToTab,
      findTabByMetadata,
      updateTabContent,
      closeAllTabs,
      primaryGroup.tabs,
      secondaryGroup.tabs,
      convertContent,
    ]);

    const prevWorkspacePathRef = useRef<string | undefined>(workspacePath);

    useEffect(() => {
      if (prevWorkspacePathRef.current && prevWorkspacePathRef.current !== workspacePath) {
        log.debug('Workspace path changed, resetting tabs', {
          from: prevWorkspacePathRef.current,
          to: workspacePath
        });
        closeAllTabs();
      }
      prevWorkspacePathRef.current = workspacePath;
    }, [workspacePath, closeAllTabs]);

    useEffect(() => {
      const removeListener = workspaceManager.addEventListener((event) => {
        if (event.type === 'workspace:closed') {
          log.debug('Workspace closed, resetting tabs');
          closeAllTabs();
        }
      });
      return () => removeListener();
    }, [closeAllTabs]);

    const handleInteraction = useCallback(async (itemId: string, userInput: string) => {
      log.debug('Panel interaction', { itemId, userInput });
    }, []);

    const handleBeforeClose = useCallback(async (_content: any) => {
      return true;
    }, []);

    return (
      <div className="bitfun-aux-pane">
        <ContentCanvas
          workspacePath={workspacePath}
          mode="agent"
          onInteraction={handleInteraction}
          onBeforeClose={handleBeforeClose}
        />
      </div>
    );
  }
);

AuxPane.displayName = 'AuxPane';

export default AuxPane;
