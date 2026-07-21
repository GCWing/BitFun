import React from 'react';

function errorText(error: unknown): string {
  if (error && typeof error === 'object') {
    const candidate = error as { stack?: unknown; message?: unknown };
    return String(candidate.stack || candidate.message || error);
  }
  return String(error || 'Canvas runtime error');
}

export function CanvasRuntimeErrorPanel({ error }: { error: unknown }) {
  return (
    <main style={{ maxWidth: 860, margin: '0 auto', padding: 12, border: '1px solid var(--border-base)', borderRadius: 8 }}>
      <h1 style={{ fontSize: 18, margin: '0 0 8px' }}>Canvas runtime error</h1>
      <pre style={{ whiteSpace: 'pre-wrap', color: 'var(--bitfun-canvas-danger)' }}>{errorText(error)}</pre>
    </main>
  );
}

export class CanvasRuntimeErrorBoundary extends React.Component<{
  children?: React.ReactNode;
  onError: (error: unknown) => void;
}, { error: unknown | null }> {
  state = { error: null };

  static getDerivedStateFromError(error: unknown) {
    return { error };
  }

  componentDidCatch(error: unknown) {
    this.props.onError(error);
  }

  render() {
    if (this.state.error) return <CanvasRuntimeErrorPanel error={this.state.error} />;
    return this.props.children;
  }
}

export function CanvasRuntimeRoot({
  component: Component,
  onReady,
  onError,
}: {
  component: React.ComponentType | null;
  onReady: () => void;
  onError: (error: unknown) => void;
}) {
  React.useEffect(() => {
    onReady();
    const timeout = window.setTimeout(onReady, 0);
    return () => window.clearTimeout(timeout);
  }, [onReady]);

  return (
    <CanvasRuntimeErrorBoundary onError={onError}>
      {Component ? <Component /> : null}
    </CanvasRuntimeErrorBoundary>
  );
}
