export type AppearancePaletteMode = 'dark' | 'light';
export type AppearancePaletteId = string;
export type ColorValue = string;

export interface BackgroundColors {
  primary: ColorValue;
  secondary: ColorValue;
  tertiary: ColorValue;
  elevated: ColorValue;
  workbench: ColorValue;
  scene: ColorValue;
  /** Persistent structural navigation/window surface. Falls back to primary for legacy packages. */
  chrome?: ColorValue;
}

export interface TextColors {
  primary: ColorValue;
  secondary: ColorValue;
  muted: ColorValue;
  disabled: ColorValue;
}

export interface AccentColors {
  50: ColorValue;
  100: ColorValue;
  200: ColorValue;
  300: ColorValue;
  400: ColorValue;
  500: ColorValue;
  600: ColorValue;
  700: ColorValue;
}

export type SecondaryAccentStop = 100 | 200 | 500 | 600;
export type SecondaryAccentColors = Pick<AccentColors, SecondaryAccentStop>;

export interface SemanticColors {
  success: ColorValue;
  successBg: ColorValue;
  successBorder: ColorValue;
  warning: ColorValue;
  warningBg: ColorValue;
  warningBorder: ColorValue;
  error: ColorValue;
  errorBg: ColorValue;
  errorBorder: ColorValue;
  info: ColorValue;
  infoBg: ColorValue;
  infoBorder: ColorValue;
}

export interface BorderColors {
  subtle: ColorValue;
  base: ColorValue;
  medium: ColorValue;
  strong: ColorValue;
  prominent: ColorValue;
}

export interface ElementBackgrounds {
  subtle: ColorValue;
  soft: ColorValue;
  base: ColorValue;
  medium: ColorValue;
  strong: ColorValue;
}

export interface GitColors {
  branch: ColorValue;
  branchBg: ColorValue;
  changes: ColorValue;
  added: ColorValue;
  deleted: ColorValue;
  staged: ColorValue;
}

export interface ScrollbarColors {
  thumb: ColorValue;
  thumbHover: ColorValue;
}

/**
 * Structural application chrome can deliberately contrast with the content
 * palette. Builtins that omit this slice keep today's single-surface behavior.
 */
export interface ChromeColors {
  background: BackgroundColors;
  text: TextColors;
  accent: AccentColors;
  border: BorderColors;
  element: ElementBackgrounds;
  scrollbar?: ScrollbarColors;
}

export interface ShadowConfig {
  xs: string;
  sm: string;
  base: string;
  lg: string;
  xl: string;
}

export interface BlurConfig {
  subtle: string;
  base: string;
}

export interface RadiusConfig {
  sm: string;
  base: string;
  lg: string;
  xl: string;
  '2xl': string;
  full: string;
}

export interface SpacingConfig {
  1: string;
  2: string;
  3: string;
  4: string;
  5: string;
  6: string;
  8: string;
  10: string;
  12: string;
  16: string;
}

export interface OpacityConfig {
  disabled: number;
  hover: number;
  focus: number;
}

export interface ButtonConfig {
  primary: {
    default: { background: ColorValue; color: ColorValue; border: ColorValue; shadow?: string };
    hover: { background: ColorValue; color: ColorValue; border: ColorValue; shadow?: string; transform?: string };
    active: { background: ColorValue; color: ColorValue; border: ColorValue; shadow?: string; transform?: string };
  };
  ghost: {
    default: { color: ColorValue };
    hover: { background: ColorValue; color: ColorValue; border: ColorValue };
  };
}

/**
 * Shared settings pages need a stable surface role that can vary independently
 * from form controls. In particular, a filled section card must not force every
 * input, divider, and focus ring to inherit the same treatment.
 */
export interface ConfigPageConfig {
  section: {
    background: ColorValue;
    border: ColorValue;
    borderWidth: string;
    shadow: string;
  };
  divider: ColorValue;
  rowHover: ColorValue;
}

export interface MotionConfig {
  instant: string;
  fast: string;
  base: string;
  slow: string;
}

export interface EasingConfig {
  standard: string;
  decelerate: string;
  smooth: string;
}

export interface MonacoEditorColors {
  background: ColorValue;
  foreground: ColorValue;
  lineHighlight: ColorValue;
  selection: ColorValue;
  cursor: ColorValue;
  [key: string]: ColorValue;
}

export interface MonacoTokenRule {
  token: string;
  foreground?: string;
  background?: string;
  fontStyle?: string;
}

export interface MonacoAppearancePaletteConfig {
  base: 'vs' | 'vs-dark' | 'hc-black' | 'hc-light';
  inherit: boolean;
  rules: MonacoTokenRule[];
  colors: MonacoEditorColors;
}

export interface AppearancePalette {
  id: AppearancePaletteId;
  name: string;
  type: AppearancePaletteMode;
  description?: string;
  author?: string;
  version?: string;
  colors: {
    background: BackgroundColors;
    text: TextColors;
    accent: AccentColors;
    purple?: SecondaryAccentColors;
    semantic: SemanticColors;
    border: BorderColors;
    element: ElementBackgrounds;
    git: GitColors;
    scrollbar?: ScrollbarColors;
    chrome?: ChromeColors;
  };
  effects: {
    shadow: ShadowConfig;
    blur: BlurConfig;
    radius: RadiusConfig;
    spacing: SpacingConfig;
    opacity: OpacityConfig;
  };
  motion: {
    duration: MotionConfig;
    easing: EasingConfig;
  };
  components?: {
    button?: ButtonConfig;
    configPage?: ConfigPageConfig;
  };
  monaco?: MonacoAppearancePaletteConfig;
  layout?: { sceneViewportBorder?: boolean };
}
