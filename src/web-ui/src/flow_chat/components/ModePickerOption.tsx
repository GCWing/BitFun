import { Tooltip } from '@/component-library';
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

  return (
    <Tooltip content={modeDescription} placement="left">
      <div
        className={`bitfun-chat-input__mode-option ${isCurrent ? 'bitfun-chat-input__mode-option--active' : ''}`}
        onClick={e => {
          e.stopPropagation();
          onSelect(modeOption.id);
        }}
      >
        <span className="bitfun-chat-input__mode-option-name">{modeName}</span>
        {isCurrent && (
          <span className="bitfun-chat-input__slash-command-current">{currentLabel}</span>
        )}
      </div>
    </Tooltip>
  );
}
