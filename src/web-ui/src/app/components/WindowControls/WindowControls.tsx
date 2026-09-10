import React from 'react';
import { Tooltip } from '@openbitfun/ui';
import { useTranslation } from 'react-i18next';
import { isWindowsDesktopRuntime } from '@/infrastructure/runtime';

// Loaded from index.html so the pre-React splash and app chrome share one stylesheet instance.

export interface WindowControlsProps
  extends Omit<React.HTMLAttributes<HTMLDivElement>, 'children'> {
  onMinimize: () => void;
  onToggleMaximize: () => void;
  onClose: () => void;
  maximized?: boolean;
  disabled?: boolean;
}

const MinimizeGlyph = () => (
  <svg width="10" height="10" viewBox="0 0 14 14" fill="none" aria-hidden="true">
    <line x1="3" y1="7" x2="11" y2="7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const MaximizeGlyph = () => (
  <svg width="10" height="10" viewBox="0 0 12 12" fill="none" aria-hidden="true">
    <rect x="2" y="2" width="8" height="8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const RestoreGlyph = () => (
  <svg width="10" height="10" viewBox="0 0 12 12" fill="none" aria-hidden="true">
    <path d="M4 4V1.5Q4 1 4.5 1h6q.5 0 .5.5v6q0 .5-.5.5H8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    <rect x="1" y="4" width="7" height="7" rx="0.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const CloseGlyph = () => (
  <svg width="10" height="10" viewBox="0 0 14 14" fill="none" aria-hidden="true">
    <line x1="3" y1="3" x2="11" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    <line x1="11" y1="3" x2="3" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const WindowsGlyph = ({ d }: { d: string }) => (
  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
    <path d={d} stroke="currentColor" strokeWidth="1" />
  </svg>
);

/** Desktop-shell window commands. This is product chrome, not a public UI primitive. */
export const WindowControls: React.FC<WindowControlsProps> = ({
  onMinimize,
  onToggleMaximize,
  onClose,
  maximized = false,
  disabled = false,
  className,
  ...props
}) => {
  const { t } = useTranslation('common');
  const isWindows = isWindowsDesktopRuntime();
  const maximizeLabel = maximized ? t('window.restore') : t('window.maximize');

  const run = (event: React.MouseEvent<HTMLButtonElement>, command: () => void) => {
    event.preventDefault();
    event.stopPropagation();
    command();
  };

  return (
    <div
      {...props}
      className={['window-controls', isWindows && 'window-controls--windows', className].filter(Boolean).join(' ')}
      data-openbitfun-component="window-controls"
      data-openbitfun-part="root"
      data-openbitfun-state={[disabled && 'disabled', maximized && 'maximized'].filter(Boolean).join(' ') || undefined}
    >
      <Tooltip content={t('window.minimize')} placement="bottom">
        <button
          type="button"
          className="window-controls__btn window-controls__btn--minimize"
          onClick={(event) => run(event, onMinimize)}
          disabled={disabled}
          aria-label={t('window.minimize')}
        >
          {isWindows ? <WindowsGlyph d="M1 6.5h10" /> : <MinimizeGlyph />}
        </button>
      </Tooltip>

      <Tooltip content={maximizeLabel} placement="bottom">
        <button
          type="button"
          className="window-controls__btn window-controls__btn--maximize"
          onClick={(event) => run(event, onToggleMaximize)}
          disabled={disabled}
          aria-label={maximizeLabel}
        >
          {isWindows ? (
            <WindowsGlyph d={maximized
              ? 'M3.5 3.5v-2h7v7h-2 M1.5 3.5h7v7h-7z'
              : 'M1.5 1.5h9v9h-9z'} />
          ) : maximized ? <RestoreGlyph /> : <MaximizeGlyph />}
        </button>
      </Tooltip>

      <Tooltip content={t('window.close')} placement="bottom">
        <button
          type="button"
          className="window-controls__btn window-controls__btn--close"
          onClick={(event) => run(event, onClose)}
          disabled={disabled}
          aria-label={t('window.close')}
        >
          {isWindows ? <WindowsGlyph d="m1.5 1.5 9 9 m0-9-9 9" /> : <CloseGlyph />}
        </button>
      </Tooltip>
    </div>
  );
};
