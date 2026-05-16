import React, { useEffect, useState } from 'react';
import { loadPrismSyntaxHighlighter } from '@/shared/utils/syntaxHighlighterLoader';

interface AsyncPrismSyntaxHighlighterProps {
  language: string;
  style: Record<string, React.CSSProperties>;
  showLineNumbers?: boolean;
  customStyle?: React.CSSProperties;
  codeTagProps?: { style?: React.CSSProperties; [key: string]: unknown };
  lineNumberStyle?: React.CSSProperties;
  children: string;
}

export const AsyncPrismSyntaxHighlighter: React.FC<AsyncPrismSyntaxHighlighterProps> = ({
  language,
  style,
  showLineNumbers,
  customStyle,
  codeTagProps,
  lineNumberStyle,
  children,
}) => {
  const [Highlighter, setHighlighter] = useState<React.ComponentType<any> | null>(null);

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
