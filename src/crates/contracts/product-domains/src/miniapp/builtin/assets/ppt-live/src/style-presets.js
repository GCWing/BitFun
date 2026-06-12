/**
 * PPT Live Style Presets
 *
 * 9 curated presets. Each preset maps to:
 * - names/descriptions: localized UI labels keyed by locale ('en-US' | 'zh-CN')
 * - styleKey: internal identifier used by deck-ai.js and the ppt-design skill
 *   (references/style-presets/<styleKey>.md)
 * - colorMode: 'light' | 'dark'
 * - palette: concrete CSS colors for HTML slide generation
 * - fontFamily: 'sans' | 'serif'
 * - density: 'spacious' | 'standard' | 'compact'
 * - keywords: regex patterns for AI style detection
 */

export const STYLE_PRESETS = {
  // === 商务/极简 ===
  'clean-business': {
    styleKey: 'clean-business',
    names: { 'en-US': 'Clean Business', 'zh-CN': '简洁商务' },
    descriptions: {
      'en-US': 'Pure white background, calm blue accent, minimal and professional',
      'zh-CN': '纯白背景，蓝色强调，极简通透的商务风格',
    },
    colorMode: 'light',
    palette: {
      background: '#ffffff',
      ink: '#1a1a1a',
      muted: '#6b7280',
      primary: '#2563eb',
      accent: '#3b82f6',
      panel: '#f8fafc',
    },
    fontFamily: 'sans',
    density: 'spacious',
    keywords: /business|clean|professional|商务|简洁|专业|企业/,
  },

  'insight-report': {
    styleKey: 'insight-report',
    names: { 'en-US': 'Insight Report', 'zh-CN': '洞察汇报' },
    descriptions: {
      'en-US': 'Consulting-grade dense pages: action titles, full tables, flowcharts, detailed breakdowns',
      'zh-CN': '咨询级高密度页面：行动式结论标题，满版表格图示，详尽逻辑拆解',
    },
    colorMode: 'light',
    palette: {
      background: '#ffffff',
      ink: '#1f2937',
      muted: '#64748b',
      primary: '#1e3a8a',
      accent: '#dc2626',
      panel: '#f1f5f9',
    },
    fontFamily: 'sans',
    density: 'compact',
    keywords: /insight|consult|academic|research|whitepaper|due.*diligence|洞察|咨询|学术|调研|详尽|深度分析|尽调/,
  },

  'minimal-gallery': {
    styleKey: 'minimal-gallery',
    names: { 'en-US': 'Monochrome Minimal', 'zh-CN': '黑白极简' },
    descriptions: {
      'en-US': 'Strict grid, monochrome palette, gallery-grade whitespace',
      'zh-CN': '严格网格，黑白灰层级，画册级留白',
    },
    colorMode: 'light',
    palette: {
      background: '#fafafa',
      ink: '#171717',
      muted: '#737373',
      primary: '#171717',
      accent: '#525252',
      panel: '#ffffff',
    },
    fontFamily: 'sans',
    density: 'spacious',
    keywords: /minimal|gallery|portfolio|grid|swiss|photo.*book|origami|极简|画廊|作品集|网格|画册|摄影|折纸/,
  },

  // === 编辑/杂志 ===
  'bold-editorial': {
    styleKey: 'bold-editorial',
    names: { 'en-US': 'Black-White-Red', 'zh-CN': '黑白红大字' },
    descriptions: {
      'en-US': 'Oversized black type on white, red accents, asymmetric editorial grid',
      'zh-CN': '白底黑色大字，红色点缀，非对称编辑排版',
    },
    colorMode: 'light',
    palette: {
      background: '#ffffff',
      ink: '#000000',
      muted: '#525252',
      primary: '#dc2626',
      accent: '#ef4444',
      panel: '#fafafa',
    },
    fontFamily: 'sans',
    density: 'spacious',
    keywords: /editorial|newspaper|bold.*type|fashion|headline|black.*white.*red|报纸|编辑|大字|头条|时尚|黑白红/,
  },

  'yellow-magazine': {
    styleKey: 'yellow-magazine',
    names: { 'en-US': 'Yellow Magazine', 'zh-CN': '黄底黑字杂志' },
    descriptions: {
      'en-US': 'High-impact yellow background, black type, handwritten accents',
      'zh-CN': '高识别度黄底黑字，手写点缀，强烈杂志感',
    },
    colorMode: 'light',
    palette: {
      background: '#facc15',
      ink: '#171717',
      muted: '#525252',
      primary: '#171717',
      accent: '#ffffff',
      panel: '#fef08a',
    },
    fontFamily: 'sans',
    density: 'standard',
    keywords: /yellow|handwritten|magazine|黄黑|黄底|手写|杂志/,
  },

  'pink-pop': {
    styleKey: 'pink-pop',
    names: { 'en-US': 'Pink Pop', 'zh-CN': '粉色波普' },
    descriptions: {
      'en-US': 'Matte pink canvas, refined magazine or street pop energy',
      'zh-CN': '哑光粉底，精致杂志或街头波普两种力度',
    },
    colorMode: 'light',
    palette: {
      background: '#fce7f3',
      ink: '#831843',
      muted: '#be185d',
      primary: '#db2777',
      accent: '#f472b6',
      panel: '#fdf2f8',
    },
    fontFamily: 'sans',
    density: 'standard',
    keywords: /pink|feminine|cute|street.*pop|粉色|粉底|女性|街头|潮流/,
  },

  // === 创意/艺术 ===
  'creative-studio': {
    styleKey: 'creative-studio',
    names: { 'en-US': 'Black-Orange Studio', 'zh-CN': '黑橙创意' },
    descriptions: {
      'en-US': 'White canvas, black type, blood-orange accents, agency sharpness',
      'zh-CN': '白底黑字血橙强调，干练时尚的创意机构风',
    },
    colorMode: 'light',
    palette: {
      background: '#ffffff',
      ink: '#171717',
      muted: '#525252',
      primary: '#ea580c',
      accent: '#c2410c',
      panel: '#fafafa',
    },
    fontFamily: 'sans',
    density: 'standard',
    keywords: /studio|creative|agency|orange|黑橙|创意|机构/,
  },

  'retro-pop': {
    styleKey: 'retro-pop',
    names: { 'en-US': 'Retro Poster Pop', 'zh-CN': '复古海报波普' },
    descriptions: {
      'en-US': 'Vintage tones, poster-grade type, playful pop-art collage',
      'zh-CN': '复古色调，海报级排版，大胆波普拼贴',
    },
    colorMode: 'light',
    palette: {
      background: '#fef3c7',
      ink: '#451a03',
      muted: '#92400e',
      primary: '#dc2626',
      accent: '#f59e0b',
      panel: '#fffbeb',
    },
    fontFamily: 'sans',
    density: 'standard',
    keywords: /retro|poster|pop.*art|vintage|sculpture|classical|复古|海报|波普|怀旧|雕塑|古典/,
  },

  'dark-neon': {
    styleKey: 'dark-neon',
    names: { 'en-US': 'Dark Neon', 'zh-CN': '暗黑霓虹' },
    descriptions: {
      'en-US': 'Deep dark canvas with neon accents: glitch art or neon blueprint',
      'zh-CN': '深色背景霓虹强调，故障艺术或霓虹制图',
    },
    colorMode: 'dark',
    palette: {
      background: '#0a0a0a',
      ink: '#e5e5e5',
      muted: '#737373',
      primary: '#22d3ee',
      accent: '#f472b6',
      panel: '#171717',
    },
    fontFamily: 'sans',
    density: 'standard',
    keywords: /dark|glitch|cyber|neon|tech.*art|blueprint|暗黑|故障|赛博|霓虹|科技|制图/,
  },

  // === 信息图 ===
  'pop-infographic': {
    styleKey: 'pop-infographic',
    names: { 'en-US': 'Pop Infographic', 'zh-CN': '波普信息图' },
    descriptions: {
      'en-US': 'Vivid pink and cyan, organic shapes or retro pixels, data-forward',
      'zh-CN': '鲜艳粉青配色，有机形态或复古像素，信息图语汇',
    },
    colorMode: 'light',
    palette: {
      background: '#ffffff',
      ink: '#171717',
      muted: '#525252',
      primary: '#ec4899',
      accent: '#06b6d4',
      panel: '#f0fdfa',
    },
    fontFamily: 'sans',
    density: 'standard',
    keywords: /infographic|data|pixel|dev|vitamin|pop|信息图|数据|像素|开发者|波普/,
  },
};

export const DEFAULT_STYLE_PRESET = 'clean-business';

export function normalizeStylePresetKey(key) {
  return key && STYLE_PRESETS[key] ? key : DEFAULT_STYLE_PRESET;
}

export function getStylePreset(key) {
  return STYLE_PRESETS[normalizeStylePresetKey(key)];
}

function resolveLocale(locale) {
  return locale === 'zh-CN' ? 'zh-CN' : 'en-US';
}

export function getStylePresetDisplayName(key, locale) {
  const preset = getStylePreset(key);
  const lang = resolveLocale(locale);
  return preset.names[lang] || preset.names['en-US'];
}

export function resolveStylePresetFromKeywords(text) {
  const raw = String(text || '').toLowerCase();
  for (const [key, preset] of Object.entries(STYLE_PRESETS)) {
    if (preset.keywords && preset.keywords.test(raw)) {
      return key;
    }
  }
  return DEFAULT_STYLE_PRESET;
}

export function getAllStylePresets(locale) {
  const lang = resolveLocale(locale);
  return Object.entries(STYLE_PRESETS).map(([key, preset]) => ({
    key,
    displayName: preset.names[lang] || preset.names['en-US'],
    description: preset.descriptions[lang] || preset.descriptions['en-US'],
    colorMode: preset.colorMode,
  }));
}
