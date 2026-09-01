let markdownMathRendererPreload: Promise<typeof import('./MarkdownMathRenderer')> | undefined;

export function preloadMarkdownMathRenderer() {
  markdownMathRendererPreload ??= import('./MarkdownMathRenderer');
  return markdownMathRendererPreload;
}
