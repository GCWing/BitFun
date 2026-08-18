export { bitfunDarkPalette } from './dark';
export { bitfunLightPalette } from './light';
export { bitfunMonochromePalette } from './monochrome';
export { bitfunMidnightPalette } from './midnight';
export { bitfunChinaStylePalette } from './chinaStyle';
export { bitfunChinaNightPalette } from './chinaNight';
export { bitfunCyberPalette } from './cyber';
export { bitfunSlatePalette } from './slate';
export { bitfunTokyoNightPalette } from './tokyoNight';

import { bitfunDarkPalette } from './dark';
import { bitfunLightPalette } from './light';
import { bitfunMonochromePalette } from './monochrome';
import { bitfunMidnightPalette } from './midnight';
import { bitfunChinaStylePalette } from './chinaStyle';
import { bitfunChinaNightPalette } from './chinaNight';
import { bitfunCyberPalette } from './cyber';
import { bitfunSlatePalette } from './slate';
import { bitfunTokyoNightPalette } from './tokyoNight';
import type { AppearancePalette, AppearancePaletteId } from './AppearancePalette';

export const DEFAULT_LIGHT_APPEARANCE_ID: AppearancePaletteId = 'bitfun-light';
export const DEFAULT_DARK_APPEARANCE_ID: AppearancePaletteId = 'bitfun-dark';

export const builtinAppearancePalettes: readonly AppearancePalette[] = Object.freeze([
  bitfunLightPalette,
  bitfunMonochromePalette,
  bitfunSlatePalette,
  bitfunDarkPalette,
  bitfunMidnightPalette,
  bitfunChinaStylePalette,
  bitfunChinaNightPalette,
  bitfunCyberPalette,
  bitfunTokyoNightPalette,
]);
