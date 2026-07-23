import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Window controls — matches BitFun main app style.
 * 32x32 transparent buttons with SVG icons, subtle hover bg.
 */
export function WindowControls() {
  const handleMinimize = () => {
    getCurrentWindow().minimize();
  };

  const handleClose = () => {
    getCurrentWindow().close();
  };

  return (
    <div className="window-controls">
      <button
        className="window-controls__btn"
        onClick={handleMinimize}
        aria-label="Minimize"
        title="Minimize"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <line x1="3" y1="7" x2="11" y2="7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </button>
      <button
        className="window-controls__btn window-controls__btn--close"
        onClick={handleClose}
        aria-label="Close"
        title="Close"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <line x1="3" y1="3" x2="11" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <line x1="11" y1="3" x2="3" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}
