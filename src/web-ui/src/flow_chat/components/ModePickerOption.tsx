import { Tooltip } from '@/component-library';
import type { KeyboardEvent } from 'react';
import type { ModeInfo } from '../reducers/modeReducer';
import { getModeDisplayDescription, getModeDisplayName } from './modeDisplay';

type Translate = (key: string, options?: { defaultValue?: string }) => string;

interface ModePickerOptionProps {
  t: Translate;
  modeOption: Pick<ModeInfo, 'id' | 'name' | 'description'>;
  currentMode: string;
  currentLabel: string;
  onSelect: (modeId: string) => void;
}

export function ModePickerOption({
  t,
  modeOption,
  currentMode,
  currentLabel,
  onSelect,
}: ModePickerOptionProps) {
  const modeDescription = getModeDisplayDescription(t, modeOption);
  const modeName = getModeDisplayName(t, modeOption);
  const isCurrent = currentMode === modeOption.id;

  const handleKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      e.stopPropagation();
      onSelect(modeOption.id);
    }
  };

  return (
    <Tooltip content={modeDescription} placement="left">
      <div
        role="option"
        tabIndex={0}
        aria-selected={isCurrent}
        aria-label={modeName}
        className={`bitfun-chat-input__mode-option ${isCurrent ? 'bitfun-chat-input__mode-option--active' : ''}`}
        onClick={e => {
          e.stopPropagation();
          onSelect(modeOption.id);
        }}
        onKeyDown={handleKeyDown}
      >
        <span className="bitfun-chat-input__mode-option-name">{modeName}</span>
        {isCurrent && (
          <span className="bitfun-chat-input__slash-command-current">{currentLabel}</span>
        )}
      </div>
    </Tooltip>
  );
}
