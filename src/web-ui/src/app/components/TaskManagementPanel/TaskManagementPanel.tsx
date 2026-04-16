/**
 * TaskManagementPanel — left-side floating task management drawer.
 *
 * Renders as a portal directly into document.body so it floats above
 * both the base session layer and any active overlay.
 *
 * Opened exclusively via the header icon button (in agent scenarios).
 * Closed by: ESC key, backdrop click, or calling closeTaskPanel.
 */

import React, { useEffect, lazy, Suspense } from 'react';
import { createPortal } from 'react-dom';
import { useSessionCapsuleStore } from '../../stores/sessionCapsuleStore';
import { ProcessingIndicator } from '@/flow_chat/components/modern/ProcessingIndicator';
import './TaskManagementPanel.scss';

const TaskDetailScene = lazy(() => import('../../scenes/task-detail/TaskDetailScene'));

const TaskManagementPanel: React.FC = () => {
  const open = useSessionCapsuleStore((s) => s.taskPanelOpen);
  const close = useSessionCapsuleStore((s) => s.closeTaskPanel);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        close();
      }
    };
    document.addEventListener('keydown', handler, true);
    return () => document.removeEventListener('keydown', handler, true);
  }, [open, close]);

  if (!open) return null;

  return createPortal(
    <div className="task-mgmt-panel-root">
      <div
        className="task-mgmt-panel-backdrop"
        aria-hidden="true"
        onClick={close}
      />
      <div
        className="task-mgmt-panel"
        role="dialog"
        aria-modal="true"
        aria-label="任务管理"
      >
        <Suspense
          fallback={
            <div className="task-mgmt-panel__loading">
              <ProcessingIndicator visible />
            </div>
          }
        >
          <TaskDetailScene />
        </Suspense>
      </div>
    </div>,
    document.body
  );
};

export default TaskManagementPanel;
