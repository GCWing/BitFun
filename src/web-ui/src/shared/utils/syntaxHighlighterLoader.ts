import type React from 'react';

type SyntaxHighlighterComponent = React.ComponentType<any>;

let prismSyntaxHighlighterPromise: Promise<SyntaxHighlighterComponent> | null = null;
let prismSyntaxHighlighterComponent: SyntaxHighlighterComponent | null = null;

export function getLoadedPrismSyntaxHighlighter(): SyntaxHighlighterComponent | null {
  return prismSyntaxHighlighterComponent;
}

export function loadPrismSyntaxHighlighter(): Promise<SyntaxHighlighterComponent> {
  prismSyntaxHighlighterPromise ??= import('react-syntax-highlighter/dist/esm/prism-async-light').then(
    (module) => {
      prismSyntaxHighlighterComponent = module.default as SyntaxHighlighterComponent;
      return prismSyntaxHighlighterComponent;
    },
  );

  return prismSyntaxHighlighterPromise;
}
