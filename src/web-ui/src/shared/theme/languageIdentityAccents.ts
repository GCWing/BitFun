// Language identity colors are data semantics, not app theme surface colors.
// Keep this registry narrow and only add values that are consumed outside the
// full language registry.
const LANGUAGE_IDENTITY_PALETTE = {
  blue: '#3178c6',
  cyan: '#00add8',
  yellow: '#f7df1e',
  orange: '#e38c00',
  red: '#ef4444',
  green: '#22c55e',
  purple: '#8b5cf6',
  slate: '#64748b',
} as const;

export const BUILTIN_LANGUAGE_ACCENTS = {
  typescript: LANGUAGE_IDENTITY_PALETTE.blue,
  typescriptReact: LANGUAGE_IDENTITY_PALETTE.cyan,
  javascript: LANGUAGE_IDENTITY_PALETTE.yellow,
  javascriptReact: LANGUAGE_IDENTITY_PALETTE.cyan,
  python: LANGUAGE_IDENTITY_PALETTE.blue,
  rust: LANGUAGE_IDENTITY_PALETTE.orange,
  go: LANGUAGE_IDENTITY_PALETTE.cyan,
  java: LANGUAGE_IDENTITY_PALETTE.orange,
  kotlin: LANGUAGE_IDENTITY_PALETTE.purple,
  cpp: LANGUAGE_IDENTITY_PALETTE.red,
  c: LANGUAGE_IDENTITY_PALETTE.slate,
  csharp: LANGUAGE_IDENTITY_PALETTE.green,
  swift: LANGUAGE_IDENTITY_PALETTE.red,
  php: LANGUAGE_IDENTITY_PALETTE.purple,
  ruby: LANGUAGE_IDENTITY_PALETTE.red,
  scala: LANGUAGE_IDENTITY_PALETTE.red,
  dart: LANGUAGE_IDENTITY_PALETTE.cyan,
  lua: LANGUAGE_IDENTITY_PALETTE.blue,
  r: LANGUAGE_IDENTITY_PALETTE.blue,
  html: LANGUAGE_IDENTITY_PALETTE.red,
  xml: LANGUAGE_IDENTITY_PALETTE.blue,
  vue: LANGUAGE_IDENTITY_PALETTE.green,
  svelte: LANGUAGE_IDENTITY_PALETTE.orange,
  css: LANGUAGE_IDENTITY_PALETTE.blue,
  scss: LANGUAGE_IDENTITY_PALETTE.purple,
  sass: LANGUAGE_IDENTITY_PALETTE.purple,
  less: LANGUAGE_IDENTITY_PALETTE.blue,
  json: LANGUAGE_IDENTITY_PALETTE.yellow,
  yaml: LANGUAGE_IDENTITY_PALETTE.red,
  toml: LANGUAGE_IDENTITY_PALETTE.orange,
  sql: LANGUAGE_IDENTITY_PALETTE.orange,
  graphql: LANGUAGE_IDENTITY_PALETTE.purple,
  dockerfile: LANGUAGE_IDENTITY_PALETTE.blue,
  makefile: LANGUAGE_IDENTITY_PALETTE.green,
  ini: LANGUAGE_IDENTITY_PALETTE.slate,
  env: LANGUAGE_IDENTITY_PALETTE.yellow,
  shell: LANGUAGE_IDENTITY_PALETTE.green,
  powershell: LANGUAGE_IDENTITY_PALETTE.blue,
  batch: LANGUAGE_IDENTITY_PALETTE.green,
  markdown: LANGUAGE_IDENTITY_PALETTE.blue,
  restructuredtext: LANGUAGE_IDENTITY_PALETTE.slate,
  image: LANGUAGE_IDENTITY_PALETTE.purple,
  audio: LANGUAGE_IDENTITY_PALETTE.green,
  video: LANGUAGE_IDENTITY_PALETTE.red,
  font: LANGUAGE_IDENTITY_PALETTE.orange,
  archive: LANGUAGE_IDENTITY_PALETTE.purple,
  binary: LANGUAGE_IDENTITY_PALETTE.slate,
  plaintext: LANGUAGE_IDENTITY_PALETTE.slate,
} as const;

export const CODE_SNIPPET_LANGUAGE_ACCENTS = {
  javascript: BUILTIN_LANGUAGE_ACCENTS.javascript,
  typescript: BUILTIN_LANGUAGE_ACCENTS.typescript,
  python: BUILTIN_LANGUAGE_ACCENTS.python,
  rust: BUILTIN_LANGUAGE_ACCENTS.rust,
  go: BUILTIN_LANGUAGE_ACCENTS.go,
  java: BUILTIN_LANGUAGE_ACCENTS.java,
  html: BUILTIN_LANGUAGE_ACCENTS.html,
  css: BUILTIN_LANGUAGE_ACCENTS.css,
  scss: BUILTIN_LANGUAGE_ACCENTS.scss,
  fallback: LANGUAGE_IDENTITY_PALETTE.slate,
} as const;

export function getCodeSnippetLanguageAccent(language?: string): string {
  if (!language) {
    return CODE_SNIPPET_LANGUAGE_ACCENTS.fallback;
  }

  return CODE_SNIPPET_LANGUAGE_ACCENTS[
    language as keyof typeof CODE_SNIPPET_LANGUAGE_ACCENTS
  ] ?? CODE_SNIPPET_LANGUAGE_ACCENTS.fallback;
}
