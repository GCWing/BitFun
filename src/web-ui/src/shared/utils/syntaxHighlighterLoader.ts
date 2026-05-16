import type React from 'react';

type SyntaxHighlighterComponent = React.ComponentType<any>;

let prismSyntaxHighlighterPromise: Promise<SyntaxHighlighterComponent> | null = null;

export function loadPrismSyntaxHighlighter(): Promise<SyntaxHighlighterComponent> {
  prismSyntaxHighlighterPromise ??= import('react-syntax-highlighter/dist/esm/prism-async-light').then(
    (module) => module.default as SyntaxHighlighterComponent,
  );

  return prismSyntaxHighlighterPromise;
}
