 

import type { Language, LanguageCategory, LanguagePlugin } from '../types';
import { createLogger } from '@/shared/utils/logger';
import { BUILTIN_LANGUAGE_ACCENTS } from '@/shared/theme/languageIdentityAccents';

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
    lspId: 'typescript',
    iconType: 'typescript',
    color: BUILTIN_LANGUAGE_ACCENTS.typescript,
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
    lspId: 'typescriptreact',
    iconType: 'react',
    color: BUILTIN_LANGUAGE_ACCENTS.typescriptReact,
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
    lspId: 'javascript',
    iconType: 'javascript',
    color: BUILTIN_LANGUAGE_ACCENTS.javascript,
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
    lspId: 'javascriptreact',
    iconType: 'react',
    color: BUILTIN_LANGUAGE_ACCENTS.javascriptReact,
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
    lspId: 'python',
    iconType: 'python',
    color: BUILTIN_LANGUAGE_ACCENTS.python,
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
    lspId: 'rust',
    iconType: 'rust',
    color: BUILTIN_LANGUAGE_ACCENTS.rust,
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
    lspId: 'go',
    iconType: 'go',
    color: BUILTIN_LANGUAGE_ACCENTS.go,
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
    lspId: 'java',
    iconType: 'java',
    color: BUILTIN_LANGUAGE_ACCENTS.java,
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
    lspId: 'kotlin',
    iconType: 'kotlin',
    color: BUILTIN_LANGUAGE_ACCENTS.kotlin,
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
    lspId: 'cpp',
    iconType: 'c-cpp',
    color: BUILTIN_LANGUAGE_ACCENTS.cpp,
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
    lspId: 'c',
    iconType: 'c-cpp',
    color: BUILTIN_LANGUAGE_ACCENTS.c,
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
    lspId: 'csharp',
    iconType: 'csharp',
    color: BUILTIN_LANGUAGE_ACCENTS.csharp,
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
    lspId: 'swift',
    iconType: 'swift',
    color: BUILTIN_LANGUAGE_ACCENTS.swift,
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
    lspId: 'php',
    iconType: 'php',
    color: BUILTIN_LANGUAGE_ACCENTS.php,
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
    lspId: 'ruby',
    iconType: 'ruby',
    color: BUILTIN_LANGUAGE_ACCENTS.ruby,
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
    lspId: 'scala',
    iconType: 'scala',
    color: BUILTIN_LANGUAGE_ACCENTS.scala,
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
    lspId: 'dart',
    iconType: 'dart',
    color: BUILTIN_LANGUAGE_ACCENTS.dart,
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
    lspId: 'lua',
    iconType: 'lua',
    color: BUILTIN_LANGUAGE_ACCENTS.lua,
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
    lspId: 'r',
    iconType: 'r',
    color: BUILTIN_LANGUAGE_ACCENTS.r,
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
    lspId: 'html',
    iconType: 'html',
    color: BUILTIN_LANGUAGE_ACCENTS.html,
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
    color: BUILTIN_LANGUAGE_ACCENTS.xml,
    supportsComments: true,
    blockComment: { start: '<!--', end: '-->' },
  },
  {
    id: 'vue',
    name: 'Vue',
    category: 'markup',
    extensions: ['vue'],
    monacoId: 'vue',
    lspId: 'vue',
    iconType: 'vue',
    color: BUILTIN_LANGUAGE_ACCENTS.vue,
    supportsComments: true,
    blockComment: { start: '<!--', end: '-->' },
  },
  {
    id: 'svelte',
    name: 'Svelte',
    category: 'markup',
    extensions: ['svelte'],
    monacoId: 'html',
    lspId: 'svelte',
    iconType: 'svelte',
    color: BUILTIN_LANGUAGE_ACCENTS.svelte,
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
    lspId: 'css',
    iconType: 'css',
    color: BUILTIN_LANGUAGE_ACCENTS.css,
    supportsComments: true,
    blockComment: { start: '/*', end: '*/' },
  },
  {
    id: 'scss',
    name: 'SCSS',
    category: 'stylesheet',
    extensions: ['scss'],
    monacoId: 'scss',
    lspId: 'scss',
    iconType: 'sass',
    color: BUILTIN_LANGUAGE_ACCENTS.scss,
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
    color: BUILTIN_LANGUAGE_ACCENTS.sass,
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
    lspId: 'less',
    iconType: 'less',
    color: BUILTIN_LANGUAGE_ACCENTS.less,
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
    color: BUILTIN_LANGUAGE_ACCENTS.json,
    supportsComments: false,
  },
  {
    id: 'yaml',
    name: 'YAML',
    category: 'data',
    extensions: ['yaml', 'yml'],
    monacoId: 'yaml',
    lspId: 'yaml',
    iconType: 'yaml',
    color: BUILTIN_LANGUAGE_ACCENTS.yaml,
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
    color: BUILTIN_LANGUAGE_ACCENTS.toml,
    supportsComments: true,
    lineCommentPrefix: '#',
  },
  {
    id: 'sql',
    name: 'SQL',
    category: 'data',
    extensions: ['sql', 'mysql', 'pgsql', 'sqlite'],
    monacoId: 'sql',
    lspId: 'sql',
    iconType: 'database',
    color: BUILTIN_LANGUAGE_ACCENTS.sql,
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
    color: BUILTIN_LANGUAGE_ACCENTS.graphql,
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
    color: BUILTIN_LANGUAGE_ACCENTS.dockerfile,
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
    color: BUILTIN_LANGUAGE_ACCENTS.makefile,
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
    color: BUILTIN_LANGUAGE_ACCENTS.ini,
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
    color: BUILTIN_LANGUAGE_ACCENTS.env,
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
    lspId: 'shellscript',
    iconType: 'shell',
    color: BUILTIN_LANGUAGE_ACCENTS.shell,
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
    lspId: 'powershell',
    iconType: 'powershell',
    color: BUILTIN_LANGUAGE_ACCENTS.powershell,
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
    color: BUILTIN_LANGUAGE_ACCENTS.batch,
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
    color: BUILTIN_LANGUAGE_ACCENTS.markdown,
    supportsComments: false,
  },
  {
    id: 'restructuredtext',
    name: 'reStructuredText',
    category: 'documentation',
    extensions: ['rst'],
    monacoId: 'restructuredtext',
    iconType: 'text',
    color: BUILTIN_LANGUAGE_ACCENTS.restructuredtext,
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
    color: BUILTIN_LANGUAGE_ACCENTS.image,
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
    color: BUILTIN_LANGUAGE_ACCENTS.audio,
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
    color: BUILTIN_LANGUAGE_ACCENTS.video,
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
    color: BUILTIN_LANGUAGE_ACCENTS.font,
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
    color: BUILTIN_LANGUAGE_ACCENTS.archive,
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
    color: BUILTIN_LANGUAGE_ACCENTS.binary,
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
    color: BUILTIN_LANGUAGE_ACCENTS.plaintext,
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
