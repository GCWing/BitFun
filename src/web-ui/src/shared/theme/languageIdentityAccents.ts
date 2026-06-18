// Language identity colors are data semantics, not app theme surface colors.
// Keep this registry narrow and only add values that are consumed outside the
// full language registry.
export const BUILTIN_LANGUAGE_ACCENTS = {
  typescript: '#3178c6',
  typescriptReact: '#61dafb',
  javascript: '#f7df1e',
  javascriptReact: '#61dafb',
  python: '#3776ab',
  rust: '#ce422b',
  go: '#00add8',
  java: '#b07219',
  kotlin: '#a97bff',
  cpp: '#f34b7d',
  c: '#555555',
  csharp: '#178600',
  swift: '#f05138',
  php: '#4f5d95',
  ruby: '#cc342d',
  scala: '#c22d40',
  dart: '#00b4ab',
  lua: '#000080',
  r: '#198ce7',
  html: '#e34c26',
  xml: '#0060ac',
  vue: '#41b883',
  svelte: '#ff3e00',
  css: '#563d7c',
  scss: '#c6538c',
  sass: '#c6538c',
  less: '#1d365d',
  json: '#cbcb41',
  yaml: '#cb171e',
  toml: '#9c4121',
  sql: '#e38c00',
  graphql: '#e10098',
  dockerfile: '#2496ed',
  makefile: '#427819',
  ini: '#d1dbe0',
  env: '#ecd53f',
  shell: '#89e051',
  powershell: '#012456',
  batch: '#c1f12e',
  markdown: '#083fa1',
  restructuredtext: '#141414',
  image: '#a855f7',
  audio: '#22c55e',
  video: '#ef4444',
  font: '#f59e0b',
  archive: '#8b5cf6',
  binary: '#64748b',
  plaintext: '#6e7781',
} as const;

export const CODE_SNIPPET_LANGUAGE_ACCENTS = {
  javascript: BUILTIN_LANGUAGE_ACCENTS.javascript,
  typescript: BUILTIN_LANGUAGE_ACCENTS.typescript,
  python: BUILTIN_LANGUAGE_ACCENTS.python,
  rust: 'var(--color-bg-primary)',
  go: BUILTIN_LANGUAGE_ACCENTS.go,
  java: '#007396',
  html: BUILTIN_LANGUAGE_ACCENTS.html,
  css: '#1572b6',
  scss: '#cc6699',
  fallback: '#858585',
} as const;

export function getCodeSnippetLanguageAccent(language?: string): string {
  if (!language) {
    return CODE_SNIPPET_LANGUAGE_ACCENTS.fallback;
  }

  return CODE_SNIPPET_LANGUAGE_ACCENTS[
    language as keyof typeof CODE_SNIPPET_LANGUAGE_ACCENTS
  ] ?? CODE_SNIPPET_LANGUAGE_ACCENTS.fallback;
}
