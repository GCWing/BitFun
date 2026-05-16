import type { ThemeId } from '../types/installer';

export type InstallerTheme = {
  id: ThemeId;
  name: string;
  type: 'dark' | 'light';
  colors: {
    background: {
      primary: string;
      secondary: string;
      tertiary: string;
      quaternary: string;
      elevated: string;
      workbench: string;
      flowchat: string;
      tooltip: string;
    };
    text: {
      primary: string;
      secondary: string;
      muted: string;
      disabled: string;
    };
    accent: Record<'50' | '100' | '200' | '300' | '400' | '500' | '600' | '700' | '800', string>;
    purple: Record<'50' | '100' | '200' | '300' | '400' | '500' | '600' | '700' | '800', string>;
    semantic: {
      success: string;
      warning: string;
      error: string;
      info: string;
      highlight: string;
      highlightBg: string;
    };
    border: {
      subtle: string;
      base: string;
      medium: string;
      strong: string;
      prominent: string;
    };
    element: {
      subtle: string;
      soft: string;
      base: string;
      medium: string;
      strong: string;
      elevated: string;
    };
  };
};

export const THEMES: InstallerTheme[] = [
  {
    id: 'bitfun-dark',
    name: 'Dark',
    type: 'dark',
    colors: {
      background: { primary: '#121214', secondary: '#18181a', tertiary: '#121214', quaternary: '#202024', elevated: '#18181a', workbench: '#121214', flowchat: '#121214', tooltip: 'rgba(30, 30, 32, 0.92)' },
      text: { primary: '#e8e8e8', secondary: '#b0b0b0', muted: '#858585', disabled: '#555555' },
      accent: { '50': 'rgba(96, 165, 250, 0.04)', '100': 'rgba(96, 165, 250, 0.08)', '200': 'rgba(96, 165, 250, 0.15)', '300': 'rgba(96, 165, 250, 0.25)', '400': 'rgba(96, 165, 250, 0.4)', '500': '#60a5fa', '600': '#3b82f6', '700': 'rgba(59, 130, 246, 0.8)', '800': 'rgba(59, 130, 246, 0.9)' },
      purple: { '50': 'rgba(139, 92, 246, 0.04)', '100': 'rgba(139, 92, 246, 0.08)', '200': 'rgba(139, 92, 246, 0.15)', '300': 'rgba(139, 92, 246, 0.25)', '400': 'rgba(139, 92, 246, 0.4)', '500': '#8b5cf6', '600': '#7c3aed', '700': 'rgba(124, 58, 237, 0.8)', '800': 'rgba(124, 58, 237, 0.9)' },
      semantic: { success: '#34d399', warning: '#f59e0b', error: '#ef4444', info: '#E1AB80', highlight: '#d4a574', highlightBg: 'rgba(212, 165, 116, 0.15)' },
      border: { subtle: 'rgba(255, 255, 255, 0.12)', base: 'rgba(255, 255, 255, 0.18)', medium: 'rgba(255, 255, 255, 0.24)', strong: 'rgba(255, 255, 255, 0.32)', prominent: 'rgba(225, 171, 128, 0.50)' },
      element: { subtle: 'rgba(255, 255, 255, 0.06)', soft: 'rgba(255, 255, 255, 0.10)', base: 'rgba(255, 255, 255, 0.13)', medium: 'rgba(255, 255, 255, 0.17)', strong: 'rgba(255, 255, 255, 0.21)', elevated: 'rgba(255, 255, 255, 0.25)' },
    },
  },
  {
    id: 'bitfun-light',
    name: 'Light',
    type: 'light',
    colors: {
      background: { primary: '#f7f8fa', secondary: '#ffffff', tertiary: '#f3f5f8', quaternary: '#ebeef3', elevated: '#ffffff', workbench: '#f7f8fa', flowchat: '#f7f8fa', tooltip: 'rgba(255, 255, 255, 0.98)' },
      text: { primary: '#1e293b', secondary: '#3d4f66', muted: '#64748b', disabled: '#94a3b8' },
      accent: { '50': 'rgba(71, 102, 143, 0.04)', '100': 'rgba(71, 102, 143, 0.08)', '200': 'rgba(71, 102, 143, 0.14)', '300': 'rgba(71, 102, 143, 0.22)', '400': 'rgba(71, 102, 143, 0.36)', '500': '#5a7bb2', '600': '#4a6694', '700': 'rgba(74, 102, 148, 0.8)', '800': 'rgba(74, 102, 148, 0.9)' },
      purple: { '50': 'rgba(107, 90, 137, 0.04)', '100': 'rgba(107, 90, 137, 0.08)', '200': 'rgba(107, 90, 137, 0.14)', '300': 'rgba(107, 90, 137, 0.22)', '400': 'rgba(107, 90, 137, 0.36)', '500': '#7c6b99', '600': '#655680', '700': 'rgba(101, 86, 128, 0.8)', '800': 'rgba(101, 86, 128, 0.9)' },
      semantic: { success: '#5b9a6f', warning: '#c08c42', error: '#c26565', info: '#5a7bb2', highlight: '#b8863a', highlightBg: 'rgba(184, 134, 58, 0.12)' },
      border: { subtle: 'rgba(100, 116, 139, 0.15)', base: 'rgba(100, 116, 139, 0.22)', medium: 'rgba(100, 116, 139, 0.32)', strong: 'rgba(100, 116, 139, 0.42)', prominent: 'rgba(100, 116, 139, 0.52)' },
      element: { subtle: 'rgba(71, 102, 143, 0.05)', soft: 'rgba(71, 102, 143, 0.08)', base: 'rgba(71, 102, 143, 0.11)', medium: 'rgba(71, 102, 143, 0.15)', strong: 'rgba(71, 102, 143, 0.20)', elevated: 'rgba(255, 255, 255, 0.92)' },
    },
  },
  {
    id: 'bitfun-midnight',
    name: 'Midnight',
    type: 'dark',
    colors: {
      background: { primary: '#2b2d30', secondary: '#1e1f22', tertiary: '#313335', quaternary: '#3c3f41', elevated: '#2b2d30', workbench: '#212121', flowchat: '#2b2d30', tooltip: 'rgba(43, 45, 48, 0.94)' },
      text: { primary: '#bcbec4', secondary: '#9da0a8', muted: '#6f737a', disabled: '#4e5157' },
      accent: { '50': 'rgba(88, 166, 255, 0.04)', '100': 'rgba(88, 166, 255, 0.08)', '200': 'rgba(88, 166, 255, 0.15)', '300': 'rgba(88, 166, 255, 0.25)', '400': 'rgba(88, 166, 255, 0.4)', '500': '#58a6ff', '600': '#3b82f6', '700': 'rgba(59, 130, 246, 0.8)', '800': 'rgba(59, 130, 246, 0.9)' },
      purple: { '50': 'rgba(156, 120, 255, 0.04)', '100': 'rgba(156, 120, 255, 0.08)', '200': 'rgba(156, 120, 255, 0.15)', '300': 'rgba(156, 120, 255, 0.25)', '400': 'rgba(156, 120, 255, 0.4)', '500': '#9c78ff', '600': '#8b5cf6', '700': 'rgba(139, 92, 246, 0.8)', '800': 'rgba(139, 92, 246, 0.9)' },
      semantic: { success: '#6aab73', warning: '#e0a055', error: '#cc7f7a', info: '#58a6ff', highlight: '#d4a574', highlightBg: 'rgba(212, 165, 116, 0.15)' },
      border: { subtle: 'rgba(255, 255, 255, 0.08)', base: 'rgba(255, 255, 255, 0.14)', medium: 'rgba(255, 255, 255, 0.20)', strong: 'rgba(255, 255, 255, 0.26)', prominent: 'rgba(255, 255, 255, 0.35)' },
      element: { subtle: 'rgba(255, 255, 255, 0.04)', soft: 'rgba(255, 255, 255, 0.06)', base: 'rgba(255, 255, 255, 0.09)', medium: 'rgba(255, 255, 255, 0.12)', strong: 'rgba(255, 255, 255, 0.15)', elevated: 'rgba(255, 255, 255, 0.18)' },
    },
  },
  {
    id: 'bitfun-china-style',
    name: 'Ink Charm',
    type: 'light',
    colors: {
      background: { primary: '#faf8f0', secondary: '#f5f3e8', tertiary: '#f0ede0', quaternary: '#ebe8d8', elevated: '#ebe9e3', workbench: '#faf8f0', flowchat: '#faf8f0', tooltip: 'rgba(250, 248, 240, 0.96)' },
      text: { primary: '#1a1a1a', secondary: '#3d3d3d', muted: '#6a6a6a', disabled: '#9a9a9a' },
      accent: { '50': 'rgba(46, 94, 138, 0.04)', '100': 'rgba(46, 94, 138, 0.08)', '200': 'rgba(46, 94, 138, 0.15)', '300': 'rgba(46, 94, 138, 0.25)', '400': 'rgba(46, 94, 138, 0.4)', '500': '#2e5e8a', '600': '#234a6d', '700': 'rgba(35, 74, 109, 0.8)', '800': 'rgba(35, 74, 109, 0.9)' },
      purple: { '50': 'rgba(126, 176, 155, 0.04)', '100': 'rgba(126, 176, 155, 0.08)', '200': 'rgba(126, 176, 155, 0.15)', '300': 'rgba(126, 176, 155, 0.25)', '400': 'rgba(126, 176, 155, 0.4)', '500': '#7eb09b', '600': '#5a9078', '700': 'rgba(90, 144, 120, 0.8)', '800': 'rgba(90, 144, 120, 0.9)' },
      semantic: { success: '#52ad5a', warning: '#f0a020', error: '#c8102e', info: '#2e5e8a', highlight: '#f0a020', highlightBg: 'rgba(240, 160, 32, 0.12)' },
      border: { subtle: 'rgba(106, 92, 70, 0.12)', base: 'rgba(106, 92, 70, 0.20)', medium: 'rgba(106, 92, 70, 0.28)', strong: 'rgba(106, 92, 70, 0.36)', prominent: 'rgba(106, 92, 70, 0.48)' },
      element: { subtle: 'rgba(46, 94, 138, 0.03)', soft: 'rgba(46, 94, 138, 0.06)', base: 'rgba(46, 94, 138, 0.10)', medium: 'rgba(46, 94, 138, 0.14)', strong: 'rgba(46, 94, 138, 0.18)', elevated: 'rgba(255, 255, 255, 0.85)' },
    },
  },
  {
    id: 'bitfun-china-night',
    name: 'Ink Night',
    type: 'dark',
    colors: {
      background: { primary: '#1a1814', secondary: '#212019', tertiary: '#262420', quaternary: '#2d2926', elevated: '#2d2926', workbench: '#1a1814', flowchat: '#1a1814', tooltip: 'rgba(26, 24, 20, 0.95)' },
      text: { primary: '#e8e6e1', secondary: '#c5c3be', muted: '#928f89', disabled: '#5f5d59' },
      accent: { '50': 'rgba(115, 165, 204, 0.04)', '100': 'rgba(115, 165, 204, 0.08)', '200': 'rgba(115, 165, 204, 0.15)', '300': 'rgba(115, 165, 204, 0.25)', '400': 'rgba(115, 165, 204, 0.4)', '500': '#73a5cc', '600': '#5a8bb3', '700': 'rgba(90, 139, 179, 0.8)', '800': 'rgba(90, 139, 179, 0.9)' },
      purple: { '50': 'rgba(150, 198, 180, 0.04)', '100': 'rgba(150, 198, 180, 0.08)', '200': 'rgba(150, 198, 180, 0.15)', '300': 'rgba(150, 198, 180, 0.25)', '400': 'rgba(150, 198, 180, 0.4)', '500': '#96c6b4', '600': '#7aab98', '700': 'rgba(122, 171, 152, 0.8)', '800': 'rgba(122, 171, 152, 0.9)' },
      semantic: { success: '#6bc072', warning: '#f5b555', error: '#e85555', info: '#73a5cc', highlight: '#e6a84a', highlightBg: 'rgba(230, 168, 74, 0.15)' },
      border: { subtle: 'rgba(232, 230, 225, 0.10)', base: 'rgba(232, 230, 225, 0.16)', medium: 'rgba(232, 230, 225, 0.22)', strong: 'rgba(232, 230, 225, 0.28)', prominent: 'rgba(232, 230, 225, 0.38)' },
      element: { subtle: 'rgba(115, 165, 204, 0.06)', soft: 'rgba(115, 165, 204, 0.09)', base: 'rgba(115, 165, 204, 0.12)', medium: 'rgba(115, 165, 204, 0.16)', strong: 'rgba(115, 165, 204, 0.20)', elevated: 'rgba(45, 41, 38, 0.95)' },
    },
  },
  {
    id: 'bitfun-cyber',
    name: 'Cyber',
    type: 'dark',
    colors: {
      background: { primary: '#101010', secondary: '#151515', tertiary: '#1a1a1a', quaternary: '#1f1f1f', elevated: '#0d0d0d', workbench: '#101010', flowchat: '#101010', tooltip: 'rgba(16, 16, 16, 0.95)' },
      text: { primary: '#e0f2ff', secondary: '#c7e7ff', muted: '#7fadcc', disabled: '#4a5a66' },
      accent: { '50': 'rgba(0, 230, 255, 0.05)', '100': 'rgba(0, 230, 255, 0.1)', '200': 'rgba(0, 230, 255, 0.18)', '300': 'rgba(0, 230, 255, 0.3)', '400': 'rgba(0, 230, 255, 0.45)', '500': '#00e6ff', '600': '#00ccff', '700': 'rgba(0, 204, 255, 0.85)', '800': 'rgba(0, 204, 255, 0.95)' },
      purple: { '50': 'rgba(138, 43, 226, 0.05)', '100': 'rgba(138, 43, 226, 0.1)', '200': 'rgba(138, 43, 226, 0.18)', '300': 'rgba(138, 43, 226, 0.3)', '400': 'rgba(138, 43, 226, 0.45)', '500': '#8a2be2', '600': '#7928ca', '700': 'rgba(121, 40, 202, 0.85)', '800': 'rgba(121, 40, 202, 0.95)' },
      semantic: { success: '#00ff9f', warning: '#ffcc00', error: '#ff0055', info: '#00e6ff', highlight: '#ffdd44', highlightBg: 'rgba(255, 221, 68, 0.15)' },
      border: { subtle: 'rgba(0, 230, 255, 0.14)', base: 'rgba(0, 230, 255, 0.20)', medium: 'rgba(0, 230, 255, 0.28)', strong: 'rgba(0, 230, 255, 0.36)', prominent: 'rgba(0, 230, 255, 0.50)' },
      element: { subtle: 'rgba(0, 230, 255, 0.06)', soft: 'rgba(0, 230, 255, 0.09)', base: 'rgba(0, 230, 255, 0.13)', medium: 'rgba(0, 230, 255, 0.17)', strong: 'rgba(0, 230, 255, 0.22)', elevated: 'rgba(0, 230, 255, 0.27)' },
    },
  },
  {
    id: 'bitfun-tokyo-night',
    name: 'Tokyo Night',
    type: 'dark',
    colors: {
      background: { primary: '#1a1b26', secondary: '#16161e', tertiary: '#14141b', quaternary: '#1e202e', elevated: '#20222c', workbench: '#16161e', flowchat: '#1a1b26', tooltip: 'rgba(22, 22, 30, 0.94)' },
      text: { primary: '#c0caf5', secondary: '#a9b1d6', muted: '#787c99', disabled: '#545c7e' },
      accent: { '50': 'rgba(122, 162, 247, 0.05)', '100': 'rgba(122, 162, 247, 0.08)', '200': 'rgba(122, 162, 247, 0.15)', '300': 'rgba(122, 162, 247, 0.25)', '400': 'rgba(122, 162, 247, 0.4)', '500': '#7aa2f7', '600': '#6183bb', '700': 'rgba(97, 131, 187, 0.85)', '800': 'rgba(97, 131, 187, 0.95)' },
      purple: { '50': 'rgba(187, 154, 247, 0.05)', '100': 'rgba(187, 154, 247, 0.08)', '200': 'rgba(187, 154, 247, 0.15)', '300': 'rgba(187, 154, 247, 0.25)', '400': 'rgba(187, 154, 247, 0.4)', '500': '#bb9af7', '600': '#9d7cd8', '700': 'rgba(157, 124, 216, 0.85)', '800': 'rgba(157, 124, 216, 0.95)' },
      semantic: { success: '#9ece6a', warning: '#e0af68', error: '#f7768e', info: '#7dcfff', highlight: '#e0af68', highlightBg: 'rgba(224, 175, 104, 0.15)' },
      border: { subtle: 'rgba(54, 59, 84, 0.45)', base: 'rgba(54, 59, 84, 0.6)', medium: 'rgba(54, 59, 84, 0.72)', strong: 'rgba(54, 59, 84, 0.85)', prominent: 'rgba(122, 162, 247, 0.45)' },
      element: { subtle: 'rgba(122, 162, 247, 0.06)', soft: 'rgba(122, 162, 247, 0.08)', base: 'rgba(122, 162, 247, 0.11)', medium: 'rgba(122, 162, 247, 0.14)', strong: 'rgba(122, 162, 247, 0.18)', elevated: 'rgba(122, 162, 247, 0.22)' },
    },
  },
  {
    id: 'bitfun-slate',
    name: 'Slate',
    type: 'dark',
    colors: {
      background: { primary: '#1a1c1e', secondary: '#1a1c1e', tertiary: '#1a1c1e', quaternary: '#32363a', elevated: '#1a1c1e', workbench: '#1a1c1e', flowchat: '#1a1c1e', tooltip: 'rgba(42, 45, 48, 0.96)' },
      text: { primary: '#eef0f3', secondary: '#c8ccd2', muted: '#9ea4ab', disabled: '#65696f' },
      accent: { '50': 'rgba(122, 176, 238, 0.04)', '100': 'rgba(122, 176, 238, 0.08)', '200': 'rgba(122, 176, 238, 0.15)', '300': 'rgba(122, 176, 238, 0.25)', '400': 'rgba(122, 176, 238, 0.4)', '500': '#7ab0ee', '600': '#689ad8', '700': 'rgba(104, 154, 216, 0.8)', '800': 'rgba(104, 154, 216, 0.9)' },
      purple: { '50': 'rgba(184, 198, 255, 0.04)', '100': 'rgba(184, 198, 255, 0.08)', '200': 'rgba(184, 198, 255, 0.15)', '300': 'rgba(184, 198, 255, 0.25)', '400': 'rgba(184, 198, 255, 0.4)', '500': '#b8c4ff', '600': '#9dacf5', '700': 'rgba(157, 172, 245, 0.8)', '800': 'rgba(157, 172, 245, 0.9)' },
      semantic: { success: '#7fb899', warning: '#d4a574', error: '#c9878d', info: '#7ab0ee', highlight: '#e2e4e7', highlightBg: 'rgba(212, 214, 216, 0.12)' },
      border: { subtle: 'rgba(255, 255, 255, 0.12)', base: 'rgba(255, 255, 255, 0.18)', medium: 'rgba(255, 255, 255, 0.24)', strong: 'rgba(255, 255, 255, 0.32)', prominent: 'rgba(255, 255, 255, 0.45)' },
      element: { subtle: 'rgba(255, 255, 255, 0.06)', soft: 'rgba(255, 255, 255, 0.10)', base: 'rgba(255, 255, 255, 0.13)', medium: 'rgba(255, 255, 255, 0.17)', strong: 'rgba(255, 255, 255, 0.21)', elevated: 'rgba(255, 255, 255, 0.25)' },
    },
  },
];

export const THEME_DISPLAY_ORDER: ThemeId[] = [
  'bitfun-light',
  'bitfun-slate',
  'bitfun-dark',
  'bitfun-midnight',
  'bitfun-china-style',
  'bitfun-china-night',
  'bitfun-cyber',
  'bitfun-tokyo-night',
];

export function findInstallerThemeById(id: ThemeId): InstallerTheme {
  return THEMES.find((t) => t.id === id)
    ?? THEMES.find((t) => t.id === 'bitfun-light')
    ?? THEMES[0];
}
