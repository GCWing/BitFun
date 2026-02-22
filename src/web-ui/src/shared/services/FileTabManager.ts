/**
 * File tab manager.
 *
 * Opens files in the editor canvas and supports optional line/range navigation.
 */
import { normalizePath } from '@/shared/utils/pathUtils';
import { getEditorType } from '@/infrastructure/language-detection';
import type { LineRange } from '@/component-library/components/Markdown';
import { createLogger } from '@/shared/utils/logger';
import { enqueuePendingTab } from './pendingTabQueue';

const log = createLogger('FileTabManager');

export interface FileTabOptions {
   
  filePath: string;
   
  fileName?: string;
   
  workspacePath?: string;
   
  jumpToLine?: number;
   
  jumpToColumn?: number;
   
  jumpToRange?: LineRange;
   
  mode?: 'agent' | 'project';
   
  forceNew?: boolean;
   
  splitView?: boolean;
   
  targetGroup?: 'primary' | 'secondary';
  /**
   * Pass `true` when the target scene was just added to openTabs (i.e. it was
   * not previously mounted).  The tab event will be enqueued instead of
   * dispatched directly, so it is processed once the scene's ContentCanvas
   * mounts and registers its event listener.
   */
  sceneJustOpened?: boolean;
}

 
class FileTabManager {
  private static instance: FileTabManager;

  private constructor() {}

  public static getInstance(): FileTabManager {
    if (!FileTabManager.instance) {
      FileTabManager.instance = new FileTabManager();
    }
    return FileTabManager.instance;
  }

   
  public openFile(options: FileTabOptions): void {
    const {
      filePath,
      fileName: providedFileName,
      workspacePath,
      jumpToLine,
      jumpToColumn,
      jumpToRange,
      mode = 'agent',
      forceNew = false,
      splitView = false,
      targetGroup = 'secondary',
      sceneJustOpened = false,
    } = options;

    
    const normalizedPath = normalizePath(filePath);
    
    
    const fileName = providedFileName || normalizedPath.split(/[/\\]/).pop() || '';
    
    
    const editorType = getEditorType(fileName);
    
    
    const finalJumpToRange = jumpToRange || (jumpToLine ? { start: jumpToLine, end: jumpToColumn ? jumpToLine : undefined } : undefined);
    
    
    const tabData = {
      filePath: normalizedPath,
      fileName,
      workspacePath,
      
      ...(finalJumpToRange && { jumpToRange: finalJumpToRange }),
      
      ...(!finalJumpToRange && jumpToLine && { jumpToLine }),
      ...(!finalJumpToRange && jumpToColumn && { jumpToColumn })
    };
    
    
    const eventDetail: Record<string, any> = {
      type: editorType,
      title: fileName,
      data: tabData,
      metadata: {
        duplicateCheckKey: normalizedPath
      },
      checkDuplicate: !forceNew,
      duplicateCheckKey: normalizedPath
    };

    
    if (splitView) {
      eventDetail.targetGroup = targetGroup;
      eventDetail.enableSplitView = true;
    }
    
    
    const eventName = mode === 'project' ? 'project-create-tab' : 'agent-create-tab';
    
    
    window.dispatchEvent(new CustomEvent('expand-right-panel'));

    // When the target scene was just added to openTabs it hasn't mounted yet,
    // so the ContentCanvas event listener doesn't exist.  Enqueue the event;
    // useTabLifecycle will drain and process it once it registers its listener.
    if (sceneJustOpened) {
      enqueuePendingTab(mode === 'project' ? 'project' : 'agent', eventDetail);
      return;
    }
    
    
    const isRightPanelCollapsed = this.isRightPanelCollapsed();
    
    if (isRightPanelCollapsed) {
      setTimeout(() => {
        window.dispatchEvent(new CustomEvent(eventName, { detail: eventDetail }));
      }, 300);
    } else {
      window.dispatchEvent(new CustomEvent(eventName, { detail: eventDetail }));
    }
  }

   
  private isRightPanelCollapsed(): boolean {
    
    try {
      const layoutState = (window as any).__BITFUN_LAYOUT_STATE__;
      return layoutState?.rightPanelCollapsed ?? false;
    } catch {
      return false;
    }
  }

   
  public openFileAndJump(
    filePath: string,
    line: number,
    column?: number,
    options?: Partial<FileTabOptions>
  ): void {
    this.openFile({
      filePath,
      jumpToLine: line,
      jumpToColumn: column,
      ...options
    });
  }

   
  public openFileAndJumpToRange(
    filePath: string,
    range: LineRange,
    options?: Partial<FileTabOptions>
  ): void {
    this.openFile({
      filePath,
      jumpToRange: range,
      ...options
    });
  }
}


export const fileTabManager = FileTabManager.getInstance();


export type { FileTabManager };
