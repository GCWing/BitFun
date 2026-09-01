import React from 'react';

interface ErrorBoundaryProps {
  children: React.ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('[ErrorBoundary]', error.message, errorInfo.componentStack);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            height: '100%',
            padding: '32px',
            textAlign: 'center',
            background: 'var(--bf-color-surface-canvas)',
            color: 'var(--bf-color-content-primary)',
            fontFamily: 'var(--bf-type-body-md-font-family)',
          }}
        >
          {/* typography-audit: allow -- warning glyph geometry, not product text */}
          <div style={{ fontSize: '48px', marginBottom: '16px' }}>⚠</div>
          <h2 style={{
            fontSize: 'var(--bf-type-flow-section-title-font-size)',
            fontWeight: 'var(--bf-type-flow-section-title-font-weight)',
            margin: '0 0 8px',
          }}>
            Something went wrong
          </h2>
          <p style={{
            fontSize: 'var(--bf-type-body-sm-font-size)',
            color: 'var(--bf-color-content-muted)',
            margin: '0 0 24px',
            maxWidth: '280px',
          }}>
            {this.state.error?.message || 'An unexpected error occurred.'}
          </p>
          <button
            onClick={this.handleRetry}
            style={{
              padding: '12px 32px',
              border: 'none',
              borderRadius: '14px',
              background: 'var(--bf-color-accent-default)',
              color: 'var(--bf-color-action-primary-content)',
              fontSize: 'var(--bf-type-body-lg-font-size)',
              fontWeight: 'var(--bf-type-label-selected-font-weight)',
              cursor: 'pointer',
            }}
          >
            Retry
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
