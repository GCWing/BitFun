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
            background: 'var(--color-bg-primary)',
            color: 'var(--color-text-primary)',
            fontFamily: 'system-ui, sans-serif',
          }}
        >
          <div style={{ fontSize: '48px', marginBottom: '16px' }}>⚠</div>
          <h2 style={{ fontSize: '18px', fontWeight: 600, margin: '0 0 8px' }}>
            Something went wrong
          </h2>
          <p style={{ fontSize: '13px', color: 'var(--color-text-muted)', margin: '0 0 24px', maxWidth: '280px' }}>
            {this.state.error?.message || 'An unexpected error occurred.'}
          </p>
          <button
            onClick={this.handleRetry}
            style={{
              padding: '12px 32px',
              border: 'none',
              borderRadius: '14px',
              background: 'var(--color-accent-500)',
              color: 'var(--btn-primary-color)',
              fontSize: '15px',
              fontWeight: 600,
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
