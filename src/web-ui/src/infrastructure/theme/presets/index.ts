 

export { bitfunDarkTheme } from './dark-theme';
export { bitfunLightTheme } from './light-theme';
export { bitfunMidnightTheme } from './midnight-theme';
export { bitfunChinaStyleTheme } from './china-style-theme';
export { bitfunChinaNightTheme } from './china-night-theme';
export { bitfunCyberTheme } from './cyber-theme';
export { bitfunSlateTheme } from './slate-theme';

import { bitfunDarkTheme } from './dark-theme';
import { bitfunLightTheme } from './light-theme';
import { bitfunMidnightTheme } from './midnight-theme';
import { bitfunChinaStyleTheme } from './china-style-theme';
import { bitfunChinaNightTheme } from './china-night-theme';
import { bitfunCyberTheme } from './cyber-theme';
import { bitfunSlateTheme } from './slate-theme';
import { ThemeConfig } from '../types';

 
export const builtinThemes: ThemeConfig[] = [
  bitfunLightTheme,
  bitfunSlateTheme,
  bitfunDarkTheme,
  bitfunMidnightTheme,
  bitfunChinaStyleTheme,
  bitfunChinaNightTheme,
  bitfunCyberTheme,
];

 
export const DEFAULT_THEME_ID = 'bitfun-slate';



