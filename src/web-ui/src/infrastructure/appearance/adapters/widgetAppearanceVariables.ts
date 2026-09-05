import { themeCssVariables } from '@openbitfun/theme-openbitfun';

import type { AppearanceThemeTokenName } from '../types';

export const WIDGET_APPEARANCE_VARIABLE_NAMES = Object.freeze(
  Object.values(themeCssVariables) as AppearanceThemeTokenName[],
);

export type WidgetAppearanceVariableName = typeof WIDGET_APPEARANCE_VARIABLE_NAMES[number];
