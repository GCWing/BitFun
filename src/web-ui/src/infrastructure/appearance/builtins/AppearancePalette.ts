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

export interface FontConfig {
  sans: string;
  mono: string;
}

export interface FontWeightConfig {
  normal: number;
  medium: number;
  semibold: number;
}

export interface FontSizeConfig {
  xs: string;
  sm: string;
  base: string;
  lg: string;
  xl: string;
  '2xl': string;
  '3xl': string;
  '4xl': string;
}

export interface LineHeightConfig {
  tight: number;
  base: number;
  relaxed: number;
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
  typography: {
    font: FontConfig;
    weight: FontWeightConfig;
    size: FontSizeConfig;
    lineHeight: LineHeightConfig;
  };
  components?: { button?: ButtonConfig };
  monaco?: MonacoAppearancePaletteConfig;
  layout?: { sceneViewportBorder?: boolean };
}
