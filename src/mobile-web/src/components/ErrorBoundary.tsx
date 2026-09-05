import React from 'react';
import { MobileButton, MobileStatus } from '@openbitfun/ui/mobile';

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
        <MobileStatus
          action={<MobileButton appearance="primary" onClick={this.handleRetry}>Retry</MobileButton>}
          description={this.state.error?.message || 'An unexpected error occurred.'}
          icon={<span>⚠</span>}
          title="Something went wrong"
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            height: '100%',
            padding: '32px',
            textAlign: 'center',
            background: 'var(--openbitfun-color-surface-canvas)',
            color: 'var(--openbitfun-color-content-primary)',
            fontFamily: 'var(--openbitfun-type-body-md-font-family)',
          }}
        />
      );
    }

    return this.props.children;
  }
}
