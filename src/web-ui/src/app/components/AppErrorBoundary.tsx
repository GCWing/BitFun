import { Component, ReactNode } from 'react';
import { Button } from '@openbitfun/ui';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';
import { buildReactCrashLogPayload } from '@/shared/utils/reactProductionError';

const log = createLogger('AppErrorBoundary');

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
  errorInfo?: any;
}

export class AppErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: any) {
    this.setState({ error, errorInfo });
    // Log every boundary capture (do not share a session-wide flag with main.tsx:
    // a second distinct error would otherwise be suppressed).
    log.error(
      '[CRASH] React error boundary caught exception',
      buildReactCrashLogPayload(error, errorInfo)
    );
  }

  handleReload = () => {
    window.location.reload();
  };

  render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    const title = i18nService.t('errors:boundary.title');
    const reloadLabel = i18nService.t('errors:boundary.reload');
    const technicalDetails = i18nService.t('errors:boundary.technicalDetails');
    const unknownError = i18nService.t('errors:boundary.unknown');
    const firstLine = this.state.error?.message?.split('\n')[0] ?? unknownError;

    return (
      <div
        style={{
          height: '100vh',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          background: 'var(--openbitfun-color-surface-workbench)',
          color: 'var(--openbitfun-color-content-primary)',
          padding: 24,
          boxSizing: 'border-box',
        }}
      >
        <div style={{ maxWidth: 760, width: '100%' }}>
          <h2 style={{
            margin: 0,
            fontFamily: 'var(--openbitfun-type-heading-page-font-family)',
            fontSize: 'var(--openbitfun-type-heading-page-font-size)',
            fontWeight: 'var(--openbitfun-type-heading-page-font-weight)',
            lineHeight: 'var(--openbitfun-type-heading-page-line-height)',
            letterSpacing: 'var(--openbitfun-type-heading-page-letter-spacing)',
          }}>{title}</h2>
          <p style={{ margin: '12px 0 0', opacity: 0.9 }}>{firstLine}</p>
          <div style={{ marginTop: 16 }}>
            <Button
              variant="fill"
              size="sm"
              onClick={this.handleReload}
            >
              {reloadLabel}
            </Button>
          </div>
          {import.meta.env.DEV && this.state.error && (
            <details style={{ marginTop: 16 }}>
              <summary style={{ cursor: 'pointer' }}>{technicalDetails}</summary>
              <pre
                style={{
                  marginTop: 12,
                  padding: 12,
                  background: 'var(--openbitfun-color-surface-panel)',
                  color: 'var(--openbitfun-color-content-secondary)',
                  borderRadius: 8,
                  overflow: 'auto',
                  maxHeight: 240,
                  fontFamily: 'var(--openbitfun-type-code-sm-font-family)',
                  fontSize: 'var(--openbitfun-type-code-sm-font-size)',
                  fontWeight: 'var(--openbitfun-type-code-sm-font-weight)',
                  lineHeight: 'var(--openbitfun-type-code-sm-line-height)',
                }}
              >
                {this.state.error.stack ?? this.state.error.message}
              </pre>
            </details>
          )}
        </div>
      </div>
    );
  }
}

export default AppErrorBoundary;
