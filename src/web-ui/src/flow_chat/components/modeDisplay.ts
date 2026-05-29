import type { ModeInfo } from '../reducers/modeReducer';

type Translate = (key: string, options?: { defaultValue?: string }) => string;

function translatedOrEmpty(t: Translate, key: string): string {
  return t(key, { defaultValue: '' });
}

export function getModeDisplayName(t: Translate, mode: Pick<ModeInfo, 'id' | 'name'>): string {
  return translatedOrEmpty(t, `chatInput.modeNames.${mode.id}`) || mode.name;
}

export function getModeDisplayDescription(
  t: Translate,
  mode: Pick<ModeInfo, 'id' | 'name' | 'description'>,
): string {
  return translatedOrEmpty(t, `chatInput.modeDescriptions.${mode.id}`) ||
    mode.description ||
    mode.name;
}
