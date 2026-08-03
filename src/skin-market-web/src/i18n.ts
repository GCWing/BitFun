import { useCallback, useEffect, useState } from 'react';

export type Locale = 'en-US' | 'zh-CN';

type MessageKey = keyof typeof messages['en-US'];
type MessageValues = Record<string, string | number>;

const LOCALE_STORAGE_KEY = 'bitfun-skin-market-locale';

const messages = {
  'en-US': {
    brand: 'BitFun',
    market: 'Skin Market',
    navBrowse: 'Browse',
    language: 'Language',
    useEnglish: 'Use English',
    useChinese: '使用中文',
    switchToLight: 'Use light theme',
    switchToDark: 'Use dark theme',
    headline: 'A different atmosphere for BitFun.',
    intro: 'Browse reviewed appearance packages, then install the one that fits your workspace.',
    desktopInstallTitle: 'Install with BitFun Desktop',
    desktopInstallNote: 'Download the package, then open Settings > Appearance in BitFun Desktop to import it.',
    catalogTitle: 'Reviewed appearances',
    catalogIntro: 'Each release is immutable, compatibility-labeled, and ready for local import.',
    searchLabel: 'Search appearances',
    searchPlaceholder: 'Search by name or description',
    modeFilterLabel: 'Color mode',
    allModes: 'All',
    lightMode: 'Light',
    darkMode: 'Dark',
    sortLabel: 'Sort',
    newest: 'Newest',
    downloads: 'Most downloaded',
    resultCount: '{count} appearances loaded',
    loadMore: 'Load more',
    loading: 'Loading appearances',
    retry: 'Try again',
    emptyTitle: 'No appearances found',
    emptyBody: 'Try a broader search or switch the color mode filter.',
    errorTitle: 'The market could not be loaded',
    errorBody: 'Check your connection and try again.',
    requestId: 'Request ID: {id}',
    openDetail: 'View {name}',
    previewAlt: '{name} appearance preview',
    previewUnavailable: 'Preview unavailable',
    mode: 'Mode',
    version: 'Version',
    by: 'By {author}',
    publishedLabel: 'Published',
    published: 'Published {date}',
    downloadCount: '{count} downloads',
    detailBack: 'Back to Skin Market',
    detailDownload: 'Download appearance',
    compatibility: 'Compatibility',
    minBitfun: 'BitFun {version} or newer',
    requiredCapabilities: 'Required capabilities',
    noExtraCapabilities: 'No additional capabilities required',
    whatsNew: 'What changed',
    releases: 'Release history',
    currentRelease: 'Current release',
    releaseNumber: 'Release {number}',
    packageSize: 'Package size',
    checksum: 'SHA-256',
    yanked: 'Unavailable',
    downloadVersion: 'Download {version}',
    olderReleases: 'Show {count} older releases',
    license: 'License',
    customLicense: 'Custom license',
    repository: 'Source repository',
    viewRepository: 'Open repository',
    packageIdentity: 'Package ID',
    notDeclared: 'Not declared',
    notFoundTitle: 'This appearance is not here',
    notFoundBody: 'The link may be outdated, or the release may no longer be public.',
    backToCatalog: 'Browse appearances',
    footerNote: 'Reviewed packages. Local installation. Your appearance stays on your device.',
  },
  'zh-CN': {
    brand: 'BitFun',
    market: 'Skin 市场',
    navBrowse: '浏览',
    language: '语言',
    useEnglish: 'Use English',
    useChinese: '使用中文',
    switchToLight: '切换到浅色模式',
    switchToDark: '切换到深色模式',
    headline: '换一种 BitFun 的气质。',
    intro: '浏览经过审核的外观包，为你的工作空间选择更合适的视觉表达。',
    desktopInstallTitle: '使用 BitFun Desktop 安装',
    desktopInstallNote: '下载外观包后，在 BitFun Desktop 中打开「设置 > 外观」并导入。',
    catalogTitle: '已审核外观',
    catalogIntro: '每个版本都锁定内容哈希，并明确标注兼容要求，可安全导入本机。',
    searchLabel: '搜索外观',
    searchPlaceholder: '搜索名称或简介',
    modeFilterLabel: '色彩模式',
    allModes: '全部',
    lightMode: '浅色',
    darkMode: '深色',
    sortLabel: '排序',
    newest: '最新发布',
    downloads: '下载最多',
    resultCount: '已加载 {count} 个外观',
    loadMore: '加载更多',
    loading: '正在加载外观',
    retry: '重试',
    emptyTitle: '没有找到外观',
    emptyBody: '可以尝试更宽泛的关键词，或切换色彩模式。',
    errorTitle: '市场暂时无法加载',
    errorBody: '请检查网络连接后重试。',
    requestId: '请求 ID：{id}',
    openDetail: '查看 {name}',
    previewAlt: '{name} 外观预览',
    previewUnavailable: '暂无预览',
    mode: '模式',
    version: '版本',
    by: '作者：{author}',
    publishedLabel: '发布时间',
    published: '发布于 {date}',
    downloadCount: '{count} 次下载',
    detailBack: '返回 Skin 市场',
    detailDownload: '下载外观包',
    compatibility: '兼容性',
    minBitfun: '需要 BitFun {version} 或更高版本',
    requiredCapabilities: '所需能力',
    noExtraCapabilities: '不需要额外能力',
    whatsNew: '更新说明',
    releases: '版本历史',
    currentRelease: '当前版本',
    releaseNumber: '第 {number} 个发布版本',
    packageSize: '安装包大小',
    checksum: 'SHA-256',
    yanked: '已停止提供',
    downloadVersion: '下载 {version}',
    olderReleases: '查看更早的 {count} 个版本',
    license: '许可',
    customLicense: '自定义许可',
    repository: '源代码仓库',
    viewRepository: '打开仓库',
    packageIdentity: '包 ID',
    notDeclared: '未声明',
    notFoundTitle: '这里没有这个外观',
    notFoundBody: '链接可能已经失效，或者该版本不再公开。',
    backToCatalog: '浏览外观',
    footerNote: '人工审核安装包，本机安装，外观资源只保存在你的设备中。',
  },
} as const;

function isLocale(value: string | null): value is Locale {
  return value === 'en-US' || value === 'zh-CN';
}

function resolveInitialLocale(): Locale {
  try {
    const stored = window.localStorage.getItem(LOCALE_STORAGE_KEY);
    if (isLocale(stored)) return stored;
  } catch {
    // Fall back to browser preference.
  }
  return navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US';
}

function interpolate(template: string, values?: MessageValues): string {
  if (!values) return template;
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    Object.prototype.hasOwnProperty.call(values, key) ? String(values[key]) : match,
  );
}

export function useI18n() {
  const [locale, setLocaleState] = useState<Locale>(resolveInitialLocale);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    try {
      window.localStorage.setItem(LOCALE_STORAGE_KEY, next);
    } catch {
      // The in-memory locale still applies when storage is unavailable.
    }
  }, []);

  const t = useCallback(
    (key: MessageKey, values?: MessageValues) => interpolate(messages[locale][key], values),
    [locale],
  );

  return { locale, setLocale, t };
}

export type Translate = ReturnType<typeof useI18n>['t'];
