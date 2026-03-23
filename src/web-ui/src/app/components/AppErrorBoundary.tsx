import { Component, ReactNode } from 'react';
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
          background: '#0b0f14',
          color: '#e5e7eb',
          padding: 24,
          boxSizing: 'border-box',
        }}
      >
        <div style={{ maxWidth: 760, width: '100%' }}>
          <h2 style={{ margin: 0, fontSize: 18, fontWeight: 600 }}>{title}</h2>
          <p style={{ margin: '12px 0 0', opacity: 0.9 }}>{firstLine}</p>
          <div style={{ marginTop: 16 }}>
            <button
              onClick={this.handleReload}
              style={{
                padding: '8px 12px',
                background: '#2563eb',
                color: '#fff',
                border: 'none',
                borderRadius: 8,
                cursor: 'pointer',
              }}
            >
              {reloadLabel}
            </button>
          </div>
          {import.meta.env.DEV && this.state.error && (
            <details style={{ marginTop: 16 }}>
              <summary style={{ cursor: 'pointer' }}>{technicalDetails}</summary>
              <pre
                style={{
                  marginTop: 12,
                  padding: 12,
                  background: '#0f172a',
                  color: '#cbd5e1',
                  borderRadius: 8,
                  overflow: 'auto',
                  maxHeight: 240,
                  fontSize: 12,
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
