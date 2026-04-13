 

import type { Language, LanguageCategory, LanguagePlugin } from '../types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('LanguageRegistry');

// ============================================================================

// ============================================================================

 
const BUILTIN_LANGUAGES: Language[] = [
  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'typescript',
    name: 'TypeScript',
    category: 'programming',
    extensions: ['ts', 'mts', 'cts', 'ets'],
    monacoId: 'typescript',
    prismId: 'typescript',
    iconType: 'typescript',
    color: '#3178c6',
    aliases: ['ts'],
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'typescript-react',
    name: 'TypeScript React',
    category: 'programming',
    extensions: ['tsx'],
    monacoId: 'typescript',
    prismId: 'tsx',
    iconType: 'react',
    color: '#61dafb',
    aliases: ['tsx'],
    parent: 'typescript',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'javascript',
    name: 'JavaScript',
    category: 'programming',
    extensions: ['js', 'mjs', 'cjs'],
    monacoId: 'javascript',
    prismId: 'javascript',
    iconType: 'javascript',
    color: '#f7df1e',
    aliases: ['js', 'es6', 'ecmascript'],
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'javascript-react',
    name: 'JavaScript React',
    category: 'programming',
    extensions: ['jsx'],
    monacoId: 'javascript',
    prismId: 'jsx',
    iconType: 'react',
    color: '#61dafb',
    aliases: ['jsx'],
    parent: 'javascript',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'python',
    name: 'Python',
    category: 'programming',
    extensions: ['py', 'pyw', 'pyi'],
    monacoId: 'python',
    iconType: 'python',
    color: '#3776ab',
    aliases: ['py', 'python3'],
    supportsComments: true,
    lineCommentPrefix: '#',
    blockComment: { start: '"""', end: '"""' },
  },
  {
    id: 'rust',
    name: 'Rust',
    category: 'programming',
    extensions: ['rs'],
    monacoId: 'rust',
    iconType: 'rust',
    color: '#ce422b',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'go',
    name: 'Go',
    category: 'programming',
    extensions: ['go'],
    monacoId: 'go',
    iconType: 'go',
    color: '#00add8',
    aliases: ['golang'],
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'java',
    name: 'Java',
    category: 'programming',
    extensions: ['java'],
    monacoId: 'java',
    iconType: 'java',
    color: '#b07219',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'kotlin',
    name: 'Kotlin',
    category: 'programming',
    extensions: ['kt', 'kts'],
    monacoId: 'kotlin',
    iconType: 'kotlin',
    color: '#a97bff',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'cpp',
    name: 'C++',
    category: 'programming',
    extensions: ['cpp', 'cxx', 'cc', 'c++', 'hpp', 'hxx', 'hh', 'h++'],
    monacoId: 'cpp',
    iconType: 'c-cpp',
    color: '#f34b7d',
    aliases: ['c++', 'cplusplus'],
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'c',
    name: 'C',
    category: 'programming',
    extensions: ['c', 'h'],
    monacoId: 'c',
    iconType: 'c-cpp',
    color: '#555555',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'csharp',
    name: 'C#',
    category: 'programming',
    extensions: ['cs', 'csx'],
    monacoId: 'csharp',
    iconType: 'csharp',
    color: '#178600',
    aliases: ['c#', 'dotnet'],
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'swift',
    name: 'Swift',
    category: 'programming',
    extensions: ['swift'],
    monacoId: 'swift',
    iconType: 'swift',
    color: '#f05138',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'php',
    name: 'PHP',
    category: 'programming',
    extensions: ['php', 'phtml', 'php3', 'php4', 'php5', 'phps'],
    monacoId: 'php',
    iconType: 'php',
    color: '#4f5d95',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'ruby',
    name: 'Ruby',
    category: 'programming',
    extensions: ['rb', 'rbw', 'rake', 'gemspec'],
    filenames: ['Rakefile', 'Gemfile'],
    monacoId: 'ruby',
    iconType: 'ruby',
    color: '#cc342d',
    supportsComments: true,
    lineCommentPrefix: '#',
    blockComment: { start: '=begin', end: '=end' },
  },
  {
    id: 'scala',
    name: 'Scala',
    category: 'programming',
    extensions: ['scala', 'sc'],
    monacoId: 'scala',
    iconType: 'scala',
    color: '#c22d40',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'dart',
    name: 'Dart',
    category: 'programming',
    extensions: ['dart'],
    monacoId: 'dart',
    iconType: 'dart',
    color: '#00b4ab',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'lua',
    name: 'Lua',
    category: 'programming',
    extensions: ['lua'],
    monacoId: 'lua',
    iconType: 'lua',
    color: '#000080',
    supportsComments: true,
    lineCommentPrefix: '--',
    blockComment: { start: '--[[', end: ']]' },
  },
  {
    id: 'r',
    name: 'R',
    category: 'programming',
    extensions: ['r', 'R', 'rmd'],
    monacoId: 'r',
    iconType: 'r',
    color: '#198ce7',
    supportsComments: true,
    lineCommentPrefix: '#',
  },

  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'html',
    name: 'HTML',
    category: 'markup',
    extensions: ['html', 'htm', 'xhtml', 'shtml'],
    monacoId: 'html',
    iconType: 'html',
    color: '#e34c26',
    supportsComments: true,
    blockComment: { start: '<!--', end: '-->' },
  },
  {
    id: 'xml',
    name: 'XML',
    category: 'markup',
    extensions: ['xml', 'xsl', 'xslt', 'xsd', 'svg', 'rss', 'atom'],
    monacoId: 'xml',
    iconType: 'xml',
    color: '#0060ac',
    supportsComments: true,
    blockComment: { start: '<!--', end: '-->' },
  },
  {
    id: 'vue',
    name: 'Vue',
    category: 'markup',
    extensions: ['vue'],
    monacoId: 'vue',
    iconType: 'vue',
    color: '#41b883',
    supportsComments: true,
    blockComment: { start: '<!--', end: '-->' },
  },
  {
    id: 'svelte',
    name: 'Svelte',
    category: 'markup',
    extensions: ['svelte'],
    monacoId: 'html',
    iconType: 'svelte',
    color: '#ff3e00',
    supportsComments: true,
    blockComment: { start: '<!--', end: '-->' },
  },

  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'css',
    name: 'CSS',
    category: 'stylesheet',
    extensions: ['css'],
    monacoId: 'css',
    iconType: 'css',
    color: '#563d7c',
    supportsComments: true,
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'scss',
    name: 'SCSS',
    category: 'stylesheet',
    extensions: ['scss'],
    monacoId: 'scss',
    iconType: 'sass',
    color: '#c6538c',
    parent: 'css',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'sass',
    name: 'Sass',
    category: 'stylesheet',
    extensions: ['sass'],
    monacoId: 'scss',
    iconType: 'sass',
    color: '#c6538c',
    parent: 'css',
    supportsComments: true,
    lineCommentPrefix: '//',
  },
  {
    id: 'less',
    name: 'Less',
    category: 'stylesheet',
    extensions: ['less'],
    monacoId: 'less',
    iconType: 'less',
    color: '#1d365d',
    parent: 'css',
    supportsComments: true,
    lineCommentPrefix: '//',
    blockComment: { start: '/*', end: '*/' },
  },

  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'json',
    name: 'JSON',
    category: 'data',
    extensions: ['json', 'jsonc', 'json5'],
    filenames: ['.babelrc', '.eslintrc', '.prettierrc', 'tsconfig.json', 'package.json'],
    monacoId: 'json',
    iconType: 'json',
    color: '#cbcb41',
    supportsComments: false,
  },
  {
    id: 'yaml',
    name: 'YAML',
    category: 'data',
    extensions: ['yaml', 'yml'],
    monacoId: 'yaml',
    iconType: 'yaml',
    color: '#cb171e',
    supportsComments: true,
    lineCommentPrefix: '#',
  },
  {
    id: 'toml',
    name: 'TOML',
    category: 'data',
    extensions: ['toml'],
    filenames: ['Cargo.toml', 'pyproject.toml'],
    monacoId: 'toml',
    iconType: 'toml',
    color: '#9c4121',
    supportsComments: true,
    lineCommentPrefix: '#',
  },
  {
    id: 'sql',
    name: 'SQL',
    category: 'data',
    extensions: ['sql', 'mysql', 'pgsql', 'sqlite'],
    monacoId: 'sql',
    iconType: 'database',
    color: '#e38c00',
    supportsComments: true,
    lineCommentPrefix: '--',
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'graphql',
    name: 'GraphQL',
    category: 'data',
    extensions: ['graphql', 'gql'],
    monacoId: 'graphql',
    iconType: 'graphql',
    color: '#e10098',
    supportsComments: true,
    lineCommentPrefix: '#',
  },

  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'dockerfile',
    name: 'Dockerfile',
    category: 'config',
    extensions: ['dockerfile'],
    filenames: ['Dockerfile', 'Dockerfile.dev', 'Dockerfile.prod'],
    monacoId: 'dockerfile',
    iconType: 'docker',
    color: '#2496ed',
    supportsComments: true,
    lineCommentPrefix: '#',
  },
  {
    id: 'makefile',
    name: 'Makefile',
    category: 'config',
    extensions: ['mk'],
    filenames: ['Makefile', 'makefile', 'GNUmakefile'],
    monacoId: 'makefile',
    iconType: 'makefile',
    color: '#427819',
    supportsComments: true,
    lineCommentPrefix: '#',
  },
  {
    id: 'ini',
    name: 'INI',
    category: 'config',
    extensions: ['ini', 'cfg', 'conf', 'properties'],
    filenames: ['.editorconfig', '.gitconfig'],
    monacoId: 'ini',
    iconType: 'config',
    color: '#d1dbe0',
    supportsComments: true,
    lineCommentPrefix: ';',
  },
  {
    id: 'env',
    name: 'Environment',
    category: 'config',
    extensions: ['env'],
    filenames: ['.env', '.env.local', '.env.development', '.env.production'],
    monacoId: 'ini',
    iconType: 'config',
    color: '#ecd53f',
    supportsComments: true,
    lineCommentPrefix: '#',
  },

  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'shell',
    name: 'Shell',
    category: 'script',
    extensions: ['sh', 'bash', 'zsh', 'fish'],
    firstLineMatch: /^#!.*\b(bash|sh|zsh|fish)\b/,
    monacoId: 'shell',
    iconType: 'shell',
    color: '#89e051',
    aliases: ['bash', 'zsh'],
    supportsComments: true,
    lineCommentPrefix: '#',
  },
  {
    id: 'powershell',
    name: 'PowerShell',
    category: 'script',
    extensions: ['ps1', 'psm1', 'psd1'],
    monacoId: 'powershell',
    iconType: 'powershell',
    color: '#012456',
    supportsComments: true,
    lineCommentPrefix: '#',
    blockComment: { start: '<#', end: '#>' },
  },
  {
    id: 'batch',
    name: 'Batch',
    category: 'script',
    extensions: ['bat', 'cmd'],
    monacoId: 'bat',
    iconType: 'batch',
    color: '#c1f12e',
    supportsComments: true,
    lineCommentPrefix: 'REM',
  },

  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'markdown',
    name: 'Markdown',
    category: 'documentation',
    extensions: ['md', 'markdown', 'mdown', 'mkd', 'mdx'],
    filenames: ['README', 'CHANGELOG', 'LICENSE'],
    monacoId: 'markdown',
    iconType: 'markdown',
    color: '#083fa1',
    supportsComments: false,
  },
  {
    id: 'restructuredtext',
    name: 'reStructuredText',
    category: 'documentation',
    extensions: ['rst'],
    monacoId: 'restructuredtext',
    iconType: 'text',
    color: '#141414',
    supportsComments: true,
    blockComment: { start: '..', end: '' },
  },

  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'image',
    name: 'Image',
    category: 'media',
    extensions: ['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'svg', 'ico', 'avif', 'tiff', 'tif'],
    monacoId: 'plaintext',
    iconType: 'image',
    color: '#a855f7',
    supportsComments: false,
  },
  
  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'audio',
    name: 'Audio',
    category: 'media',
    extensions: ['mp3', 'wav', 'flac', 'aac', 'ogg', 'm4a', 'wma', 'aiff'],
    monacoId: 'plaintext',
    iconType: 'audio',
    color: '#22c55e',
    supportsComments: false,
  },
  
  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'video',
    name: 'Video',
    category: 'media',
    extensions: ['mp4', 'avi', 'mkv', 'mov', 'wmv', 'flv', 'webm', 'm4v', 'mpeg', 'mpg'],
    monacoId: 'plaintext',
    iconType: 'video',
    color: '#ef4444',
    supportsComments: false,
  },
  
  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'font',
    name: 'Font',
    category: 'media',
    extensions: ['ttf', 'otf', 'woff', 'woff2', 'eot'],
    monacoId: 'plaintext',
    iconType: 'font',
    color: '#f59e0b',
    supportsComments: false,
  },
  
  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'archive',
    name: 'Archive',
    category: 'binary',
    extensions: ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz', 'dmg', 'iso', 'tgz'],
    monacoId: 'plaintext',
    iconType: 'archive',
    color: '#8b5cf6',
    supportsComments: false,
  },
  
  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'binary',
    name: 'Binary',
    category: 'binary',
    extensions: ['exe', 'dll', 'so', 'dylib', 'bin', 'dat', 'o', 'a', 'lib'],
    monacoId: 'plaintext',
    iconType: 'binary',
    color: '#64748b',
    supportsComments: false,
  },

  // -------------------------------------------------------------------------
  
  // -------------------------------------------------------------------------
  {
    id: 'plaintext',
    name: 'Plain Text',
    category: 'other',
    extensions: ['txt', 'text', 'log'],
    monacoId: 'plaintext',
    iconType: 'text',
    color: '#6e7681',
    supportsComments: false,
  },
];

// ============================================================================

// ============================================================================

 
class LanguageRegistry {
  private static instance: LanguageRegistry;
  
   
  private languages = new Map<string, Language>();
  
   
  private extensionIndex = new Map<string, Language[]>();
  
   
  private filenameIndex = new Map<string, Language>();
  
   
  private aliasIndex = new Map<string, Language>();
  
   
  private monacoIdIndex = new Map<string, Language[]>();
  
   
  private plugins: LanguagePlugin[] = [];
  
  private constructor() {
    this.initBuiltinLanguages();
  }
  
   
  public static getInstance(): LanguageRegistry {
    if (!LanguageRegistry.instance) {
      LanguageRegistry.instance = new LanguageRegistry();
    }
    return LanguageRegistry.instance;
  }
  
   
  private initBuiltinLanguages(): void {
    BUILTIN_LANGUAGES.forEach(lang => this.register(lang));
    log.debug('Initialized', { languageCount: this.languages.size });
  }
  
   
  public register(language: Language): void {
    
    this.languages.set(language.id, language);
    
    
    language.extensions.forEach(ext => {
      const existing = this.extensionIndex.get(ext) || [];
      existing.push(language);
      this.extensionIndex.set(ext, existing);
    });
    
    
    language.filenames?.forEach(filename => {
      this.filenameIndex.set(filename.toLowerCase(), language);
    });
    
    
    language.aliases?.forEach(alias => {
      this.aliasIndex.set(alias.toLowerCase(), language);
    });
    
    
    const monacoLangs = this.monacoIdIndex.get(language.monacoId) || [];
    monacoLangs.push(language);
    this.monacoIdIndex.set(language.monacoId, monacoLangs);
  }
  
   
  public registerPlugin(plugin: LanguagePlugin): void {
    this.plugins.push(plugin);
    
    
    plugin.getLanguages().forEach(lang => this.register(lang));
    
    log.debug('Plugin registered', { pluginName: plugin.name });
  }
  
   
  public getById(id: string): Language | undefined {
    return this.languages.get(id) || this.aliasIndex.get(id.toLowerCase());
  }
  
   
  public getByExtension(extension: string): Language[] {
    const ext = extension.toLowerCase().replace(/^\./, '');
    return this.extensionIndex.get(ext) || [];
  }
  
   
  public getByFilename(filename: string): Language | undefined {
    return this.filenameIndex.get(filename.toLowerCase());
  }
  
   
  public getByMonacoId(monacoId: string): Language[] {
    return this.monacoIdIndex.get(monacoId) || [];
  }
  
   
  public getAll(): Language[] {
    return Array.from(this.languages.values());
  }
  
   
  public getByCategory(category: LanguageCategory): Language[] {
    return this.getAll().filter(lang => lang.category === category);
  }
  
   
  public getDefault(): Language {
    return this.languages.get('plaintext')!;
  }
  
   
  public has(id: string): boolean {
    return this.languages.has(id) || this.aliasIndex.has(id.toLowerCase());
  }
  
   
  public getStats(): {
    totalLanguages: number;
    byCategory: Record<LanguageCategory, number>;
    pluginCount: number;
  } {
    const byCategory: Record<string, number> = {};
    
    this.getAll().forEach(lang => {
      byCategory[lang.category] = (byCategory[lang.category] || 0) + 1;
    });
    
    return {
      totalLanguages: this.languages.size,
      byCategory: byCategory as Record<LanguageCategory, number>,
      pluginCount: this.plugins.length,
    };
  }
}


export const languageRegistry = LanguageRegistry.getInstance();
export default LanguageRegistry;
