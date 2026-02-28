 



 
export type ThemeType = 'dark' | 'light';

 
export type ColorValue = string; // hex, rgb, rgba, hsl, hsla

 
export type ThemeId = string;



 
export interface BackgroundColors {
  primary: ColorValue;
  secondary: ColorValue;
  tertiary: ColorValue;
  quaternary: ColorValue;
  elevated: ColorValue;
  workbench: ColorValue;
  /** Scene panel background — used by SceneViewport, FlowChat, and all scene content areas. */
  scene: ColorValue;
  tooltip?: ColorValue;
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
  800: ColorValue;
}

 
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
  
  
  highlight: ColorValue;
  highlightBg: ColorValue;
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
  elevated: ColorValue;
}

 
export interface GitColors {
  branch: ColorValue;
  branchBg: ColorValue;
  changes: ColorValue;
  changesBg: ColorValue;
  added: ColorValue;
  addedBg: ColorValue;
  deleted: ColorValue;
  deletedBg: ColorValue;
  staged: ColorValue;
  stagedBg: ColorValue;
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
  '2xl': string;
}

 
export interface GlowConfig {
  blue: string;
  purple: string;
  mixed: string;
}

 
export interface BlurConfig {
  subtle: string;
  base: string;
  medium: string;
  strong: string;
  intense: string;
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
  overlay: number;
}



 
export interface ButtonConfig {
  
  default: {
    background: ColorValue;
    color: ColorValue;
    border: ColorValue;
    shadow?: string;
  };
  
  
  hover: {
    background: ColorValue;
    color: ColorValue;
    border: ColorValue;
    shadow?: string;
    transform?: string;
  };
  
  
  active: {
    background: ColorValue;
    color: ColorValue;
    border: ColorValue;
    shadow?: string;
    transform?: string;
  };
  
  
  primary: {
    default: {
      background: ColorValue;
      color: ColorValue;
      border: ColorValue;
      shadow?: string;
    };
    hover: {
      background: ColorValue;
      color: ColorValue;
      border: ColorValue;
      shadow?: string;
      transform?: string;
    };
    active: {
      background: ColorValue;
      color: ColorValue;
      border: ColorValue;
      shadow?: string;
      transform?: string;
    };
  };
  
  
  ghost: {
    default: {
      background: ColorValue;
      color: ColorValue;
      border: ColorValue;
      shadow?: string;
    };
    hover: {
      background: ColorValue;
      color: ColorValue;
      border: ColorValue;
      shadow?: string;
      transform?: string;
    };
    active: {
      background: ColorValue;
      color: ColorValue;
      border: ColorValue;
      shadow?: string;
      transform?: string;
    };
  };
}

 
export interface WindowControlsConfig {
  
  minimize: {
    dot: ColorValue;           
    dotShadow?: string;        
    hoverBg: ColorValue;       
    hoverColor: ColorValue;    
    hoverBorder: ColorValue;   
    hoverShadow?: string;      
  };
  
  
  maximize: {
    dot: ColorValue;           
    dotShadow?: string;        
    hoverBg: ColorValue;       
    hoverColor: ColorValue;    
    hoverBorder: ColorValue;   
    hoverShadow?: string;      
  };
  
  
  close: {
    dot: ColorValue;           
    dotShadow?: string;        
    hoverBg: ColorValue;       
    hoverColor: ColorValue;    
    hoverBorder: ColorValue;   
    hoverShadow?: string;      
  };
  
  
  common: {
    defaultColor: ColorValue;          
    defaultDot: ColorValue;            
    disabledDot: ColorValue;           
    flowGradient?: string;             
  };
}



 
export interface MotionConfig {
  instant: string;
  fast: string;
  base: string;
  slow: string;
  lazy: string;
}

 
export interface EasingConfig {
  standard: string;
  decelerate: string;
  accelerate: string;
  bounce: string;
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
  bold: number;
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
  '5xl': string;
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

 
export interface MonacoThemeConfig {
  base: 'vs' | 'vs-dark' | 'hc-black' | 'hc-light';
  inherit: boolean;
  rules: MonacoTokenRule[];
  colors: MonacoEditorColors;
}



 
export interface ThemeConfig {
  
  id: ThemeId;
  name: string;
  type: ThemeType;
  description?: string;
  author?: string;
  version?: string;
  
  
  colors: {
    background: BackgroundColors;
    text: TextColors;
    accent: AccentColors;
    purple?: AccentColors; 
    semantic: SemanticColors;
    border: BorderColors;
    element: ElementBackgrounds;
    git: GitColors;
    scrollbar?: ScrollbarColors; 
  };
  
  
  effects: {
    shadow: ShadowConfig;
    glow: GlowConfig;
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
  
  
  components?: {
    button?: ButtonConfig;
    windowControls?: WindowControlsConfig;
  };
  
  // Monaco Editor
  monaco?: MonacoThemeConfig;
}



 
export interface ThemeMetadata {
  id: ThemeId;
  name: string;
  type: ThemeType;
  description?: string;
  author?: string;
  version?: string;
  builtin: boolean; 
  createdAt?: string;
  updatedAt?: string;
  thumbnail?: string; 
}



 
export interface ThemeExport {
  schema: string; 
  theme: ThemeConfig;
  metadata: ThemeMetadata;
  exportedAt: string;
}

 
export interface VSCodeThemeImport {
  name: string;
  type: ThemeType;
  colors: Record<string, string>;
  tokenColors: Array<{
    scope: string | string[];
    settings: {
      foreground?: string;
      background?: string;
      fontStyle?: string;
    };
  }>;
}



 
export interface ThemeValidationResult {
  valid: boolean;
  errors: Array<{
    path: string;
    message: string;
    code: string;
  }>;
  warnings: Array<{
    path: string;
    message: string;
    code: string;
  }>;
}



 
export type ThemeEventType = 
  | 'theme:before-change'
  | 'theme:after-change'
  | 'theme:load'
  | 'theme:unload'
  | 'theme:register'
  | 'theme:unregister';

 
export interface ThemeEvent {
  type: ThemeEventType;
  themeId: ThemeId;
  theme?: ThemeConfig;
  previousTheme?: ThemeConfig;
  timestamp: number;
}

 
export type ThemeEventListener = (event: ThemeEvent) => void | Promise<void>;



 
export interface ThemeHooks {
  beforeChange?: (newTheme: ThemeConfig, oldTheme?: ThemeConfig) => void | Promise<void>;
  afterChange?: (theme: ThemeConfig, oldTheme?: ThemeConfig) => void | Promise<void>;
  onLoad?: (theme: ThemeConfig) => void | Promise<void>;
  onUnload?: (theme: ThemeConfig) => void | Promise<void>;
}



 
export interface ThemeAdapter {
  name: string;
  supports: (data: any) => boolean;
  convert: (data: any) => ThemeConfig;
}



 
export interface ColorPreset {
  id: string;
  name: string;
  colors: Partial<ThemeConfig['colors']>;
}

 
export interface EffectPreset {
  id: string;
  name: string;
  effects: Partial<ThemeConfig['effects']>;
}
