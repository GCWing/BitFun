import React from 'react';
import { DesignSystemProvider, ThemeRoot } from '@openbitfun/ui';

function errorText(error: unknown): string {
  if (error && typeof error === 'object') {
    const candidate = error as { stack?: unknown; message?: unknown };
    return String(candidate.stack || candidate.message || error);
  }
  return String(error || 'Canvas runtime error');
}

export function CanvasRuntimeErrorPanel({ error }: { error: unknown }) {
  return (
    <main style={{ maxWidth: 860, margin: '0 auto', padding: 12, border: '1px solid var(--openbitfun-color-border-default)', borderRadius: 8 }}>
      <h1 style={{
        fontSize: 'var(--openbitfun-type-heading-page-font-size)',
        fontWeight: 'var(--openbitfun-type-heading-page-font-weight)',
        lineHeight: 'var(--openbitfun-type-heading-page-line-height)',
        margin: '0 0 8px',
      }}>Canvas runtime error</h1>
      <pre style={{ whiteSpace: 'pre-wrap', color: 'var(--openbitfun-canvas-danger)' }}>{errorText(error)}</pre>
    </main>
  );
}

export class CanvasRuntimeErrorBoundary extends React.Component<{
  children?: React.ReactNode;
  onError: (error: unknown, componentStack?: string) => void;
}, { error: unknown | null }> {
  state = { error: null };

  static getDerivedStateFromError(error: unknown) {
    return { error };
  }

  componentDidCatch(error: unknown, info: React.ErrorInfo) {
    this.props.onError(error, info.componentStack ?? undefined);
  }

  render() {
    if (this.state.error) return <CanvasRuntimeErrorPanel error={this.state.error} />;
    return this.props.children;
  }
}

function CanvasCommitProbe({ onReady }: { onReady: () => void }) {
  React.useLayoutEffect(() => {
    let cancelled = false;
    let secondFrame = 0;
    const firstFrame = window.requestAnimationFrame(() => {
      secondFrame = window.requestAnimationFrame(() => {
        if (!cancelled) onReady();
      });
    });
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(firstFrame);
      if (secondFrame) window.cancelAnimationFrame(secondFrame);
    };
  }, [onReady]);
  return null;
}

export function CanvasRuntimeRoot({
  component: Component,
  onReady,
  onError,
}: {
  component: React.ComponentType | null;
  onReady: () => void;
  onError: (error: unknown, componentStack?: string) => void;
}) {
  const colorScheme = document.documentElement.style.colorScheme === 'dark' ? 'dark' : 'light';

  return (
    <CanvasRuntimeErrorBoundary onError={onError}>
      <DesignSystemProvider colorScheme={colorScheme} portalHost={document.body}>
        <ThemeRoot colorScheme={colorScheme} className="openbitfun-canvas-design-system-root">
          {Component ? <Component /> : null}
          <CanvasCommitProbe onReady={onReady} />
        </ThemeRoot>
      </DesignSystemProvider>
    </CanvasRuntimeErrorBoundary>
  );
}
