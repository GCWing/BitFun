// Language identity colors are data semantics, not app theme surface colors.
// Keep this registry narrow and only add values that are consumed outside the
// full language registry.
export const CODE_SNIPPET_LANGUAGE_ACCENTS = {
  javascript: '#f7df1e',
  typescript: '#3178c6',
  python: '#3776ab',
  rust: 'var(--color-bg-primary)',
  go: '#00add8',
  java: '#007396',
  html: '#e34c26',
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
