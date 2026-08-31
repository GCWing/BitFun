import React from 'react';
import { Tooltip } from '@bitfun/ui';
import { useTranslation } from 'react-i18next';
import './WindowControls.scss';

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
  const maximizeLabel = maximized ? t('window.restore') : t('window.maximize');

  const run = (event: React.MouseEvent<HTMLButtonElement>, command: () => void) => {
    event.preventDefault();
    event.stopPropagation();
    command();
  };

  return (
    <div
      {...props}
      className={['window-controls', className].filter(Boolean).join(' ')}
      data-bf-component="window-controls"
      data-bf-part="root"
      data-bf-state={[disabled && 'disabled', maximized && 'maximized'].filter(Boolean).join(' ') || undefined}
    >
      <Tooltip content={t('window.minimize')} placement="bottom">
        <button
          type="button"
          className="window-controls__btn window-controls__btn--minimize"
          onClick={(event) => run(event, onMinimize)}
          disabled={disabled}
          aria-label={t('window.minimize')}
        >
          <MinimizeGlyph />
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
          {maximized ? <RestoreGlyph /> : <MaximizeGlyph />}
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
          <CloseGlyph />
        </button>
      </Tooltip>
    </div>
  );
};
