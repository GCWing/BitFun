import React, { useEffect, useState } from 'react';
import { getLoadedPrismSyntaxHighlighter, loadPrismSyntaxHighlighter } from '@/shared/utils/syntaxHighlighterLoader';
import type { FlowCodeBlockFallbackProps } from './Markdown';

interface AsyncPrismSyntaxHighlighterProps {
  language: string;
  style: Record<string, React.CSSProperties>;
  showLineNumbers?: boolean;
  customStyle?: React.CSSProperties;
  codeTagProps?: { style?: React.CSSProperties; [key: string]: unknown };
  lineNumberStyle?: React.CSSProperties;
  fallback?: React.ComponentType<FlowCodeBlockFallbackProps>;
  fallbackProps?: FlowCodeBlockFallbackProps;
  children: string;
}

export const AsyncPrismSyntaxHighlighter: React.FC<AsyncPrismSyntaxHighlighterProps> = ({
  language,
  style,
  showLineNumbers,
  customStyle,
  codeTagProps,
  lineNumberStyle,
  fallback: Fallback,
  fallbackProps,
  children,
}) => {
  const [Highlighter, setHighlighter] = useState<React.ComponentType<any> | null>(() => getLoadedPrismSyntaxHighlighter());

  useEffect(() => {
    let cancelled = false;
    void loadPrismSyntaxHighlighter()
      .then((component) => {
        if (!cancelled) {
          setHighlighter(() => component);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setHighlighter(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  if (!Highlighter) {
    if (Fallback && fallbackProps) {
      return <Fallback {...fallbackProps} />;
    }

    return (
      <pre
        className={`language-${language} code-block-fallback`}
        style={customStyle}
      >
        <code style={codeTagProps?.style}>{children}</code>
      </pre>
    );
  }

  return (
    <Highlighter
      language={language}
      style={style}
      showLineNumbers={showLineNumbers}
      customStyle={customStyle}
      codeTagProps={codeTagProps}
      lineNumberStyle={lineNumberStyle}
    >
      {children}
    </Highlighter>
  );
};
