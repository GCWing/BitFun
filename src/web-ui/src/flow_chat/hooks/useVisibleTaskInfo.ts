/**
 * Hook to detect the nearest Task tool item above the current viewport.
 * Used to show a sticky indicator of which Task the user is currently reading.
 *
 * - Scans virtualItems for FlowToolItem entries with toolName === 'Task'.
 * - Uses the DOM position of rendered Task items to determine which one is
 *   just above the viewport top.
 * - Returns the Task description and a callback to scroll to it.
 */

import { useRef, useCallback, useState, useEffect } from 'react';
import type { VirtualItem } from '../store/modernFlowChatStore';
import type { FlowToolItem } from '../types/flow-chat';

const VIEWPORT_TOP_OFFSET_PX = 57; // Keep in sync with PINNED_TURN_VIEWPORT_OFFSET_PX.
const TASK_TOOL_NAME = 'Task';

export interface VisibleTaskInfo {
  /** The virtual item index of the Task tool. */
  virtualIndex: number;
  /** The FlowItem id of the Task tool. */
  itemId: string;
  /** Display label for the Task (description or prompt). */
  label: string;
  /** The turnId this Task belongs to. */
  turnId: string;
}

interface UseVisibleTaskInfoOptions {
  scrollerRef: React.RefObject<HTMLElement | null>;
  virtualItems: VirtualItem[];
}

interface UseVisibleTaskInfoReturn {
  visibleTaskInfo: VisibleTaskInfo | null;
  /** Scroll the list so the indicated Task is at the viewport top. */
  scrollToTask: () => void;
}

function getTaskLabel(toolItem: FlowToolItem): string {
  const input = toolItem.toolCall?.input;
  if (!input) return '';
  const desc = input.description || input.prompt || input.task || '';
  return typeof desc === 'string' ? desc.trim() : '';
}

function findTaskVirtualItems(virtualItems: VirtualItem[]): Array<{
  index: number;
  itemId: string;
  turnId: string;
  label: string;
}> {
  const result: Array<{ index: number; itemId: string; turnId: string; label: string }> = [];

  for (let i = 0; i < virtualItems.length; i++) {
    const vItem = virtualItems[i];
    if (vItem.type !== 'model-round') continue;

    const round = vItem.data;
    for (const flowItem of round.items) {
      if (flowItem.type === 'tool' && (flowItem as FlowToolItem).toolName === TASK_TOOL_NAME) {
        result.push({
          index: i,
          itemId: flowItem.id,
          turnId: vItem.turnId,
          label: getTaskLabel(flowItem as FlowToolItem),
        });
      }
    }
  }

  return result;
}

export function useVisibleTaskInfo(options: UseVisibleTaskInfoOptions): UseVisibleTaskInfoReturn {
  const { scrollerRef, virtualItems } = options;
  const [visibleTaskInfo, setVisibleTaskInfo] = useState<VisibleTaskInfo | null>(null);
  const lastVisibleRef = useRef<VisibleTaskInfo | null>(null);
  const taskItemsRef = useRef(findTaskVirtualItems(virtualItems));

  // Keep task items cache in sync without triggering re-renders.
  useEffect(() => {
    taskItemsRef.current = findTaskVirtualItems(virtualItems);
  }, [virtualItems]);

  const checkVisibleTask = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;

    const taskItems = taskItemsRef.current;
    if (taskItems.length === 0) {
      if (lastVisibleRef.current !== null) {
        lastVisibleRef.current = null;
        setVisibleTaskInfo(null);
      }
      return;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const viewportTop = scrollerRect.top + VIEWPORT_TOP_OFFSET_PX;

    // Find the last Task whose DOM element top is above the viewport top.
    let matched: VisibleTaskInfo | null = null;

    for (let i = taskItems.length - 1; i >= 0; i--) {
      const task = taskItems[i];
      const element = scroller.querySelector<HTMLElement>(
        `.flowchat-flow-item[data-flow-item-id="${CSS.escape(task.itemId)}"][data-tool-name="${TASK_TOOL_NAME}"]`,
      );
      if (!element) continue;

      const rect = element.getBoundingClientRect();
      // Task element must be above or crossing the viewport top.
      if (rect.top <= viewportTop) {
        matched = {
          virtualIndex: task.index,
          itemId: task.itemId,
          label: task.label,
          turnId: task.turnId,
        };
        break;
      }
    }

    if (
      matched?.itemId !== lastVisibleRef.current?.itemId ||
      matched?.label !== lastVisibleRef.current?.label
    ) {
      lastVisibleRef.current = matched;
      setVisibleTaskInfo(matched);
    }
  }, [scrollerRef]);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;

    let rafId: number | null = null;
    const throttledCheck = () => {
      if (rafId) return;
      rafId = requestAnimationFrame(() => {
        checkVisibleTask();
        rafId = null;
      });
    };

    scroller.addEventListener('scroll', throttledCheck, { passive: true });
    checkVisibleTask();

    return () => {
      scroller.removeEventListener('scroll', throttledCheck);
      if (rafId) cancelAnimationFrame(rafId);
    };
  }, [checkVisibleTask, scrollerRef]);

  // Reset when session / items change.
  useEffect(() => {
    lastVisibleRef.current = null;
    setVisibleTaskInfo(null);
  }, [virtualItems]);

  const scrollToTask = useCallback(() => {
    const info = lastVisibleRef.current ?? visibleTaskInfo;
    if (!info) return;

    const scroller = scrollerRef.current;
    if (!scroller) return;

    const element = scroller.querySelector<HTMLElement>(
      `.flowchat-flow-item[data-flow-item-id="${CSS.escape(info.itemId)}"][data-tool-name="${TASK_TOOL_NAME}"]`,
    );
    if (!element) return;

    const scrollerRect = scroller.getBoundingClientRect();
    const elementRect = element.getBoundingClientRect();
    const offset = elementRect.top - scrollerRect.top - VIEWPORT_TOP_OFFSET_PX + scroller.scrollTop;

    scroller.scrollTo({ top: offset, behavior: 'smooth' });
  }, [visibleTaskInfo, scrollerRef]);

  return {
    visibleTaskInfo,
    scrollToTask,
  };
}
