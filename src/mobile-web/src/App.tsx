import React, { Suspense, lazy, useState, useCallback, useRef, useEffect } from 'react';
import PairingPage from './pages/PairingPage';
import WorkspacePage from './pages/WorkspacePage';
import SessionListPage from './pages/SessionListPage';
import DevicesPage from './pages/DevicesPage';
import { ErrorBoundary } from './components/ErrorBoundary';
import { I18nProvider, useI18n } from './i18n';
import { RelayHttpClient } from './services/RelayHttpClient';
import { RemoteSessionManager } from './services/RemoteSessionManager';
import { ThemeProvider } from './theme';
import { useConnectionHealth } from './hooks/useConnectionHealth';
import { useMobileStore } from './services/store';
import './styles/index.scss';

type Page = 'pairing' | 'workspace' | 'sessions' | 'chat' | 'devices';
type NavDirection = 'push' | 'pop' | null;

const NAV_DURATION = 300;
const ChatPage = lazy(() => import('./pages/ChatPage'));

function getNavClass(
  targetPage: Page,
  currentPage: Page,
  navDir: NavDirection,
  isAnimating: boolean,
): string {
  if (!isAnimating) return '';
  const isEntering = currentPage === targetPage;
  if (isEntering) {
    return navDir === 'push' ? 'nav-push-enter' : 'nav-pop-enter';
  }
  return navDir === 'push' ? 'nav-push-exit' : 'nav-pop-exit';
}

const AppContent: React.FC = () => {
  const { t } = useI18n();
  const [page, setPage] = useState<Page>('pairing');
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeSessionName, setActiveSessionName] = useState<string>('Session');
  const [chatAutoFocus, setChatAutoFocus] = useState(false);
  const connectionHealth = useMobileStore((state) => state.connectionHealth);
  const clientRef = useRef<RelayHttpClient | null>(null);
  const sessionMgrRef = useRef<RemoteSessionManager | null>(null);
  const [sessionMgr, setSessionMgr] = useState<RemoteSessionManager | null>(null);

  useConnectionHealth(sessionMgr);

  const [navDir, setNavDir] = useState<NavDirection>(null);
  const [prevPage, setPrevPage] = useState<Page | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  // Track the page stack for browser history integration.
  // When user triggers browser back (phone back button / edge swipe),
  // we intercept popstate and perform in-app navigation instead.
  const pageStackRef = useRef<Page[]>(['pairing']);
  const isPopstateNavRef = useRef(false);

  const navigateTo = useCallback((target: Page, direction: NavDirection) => {
    setPage(prev => {
      setPrevPage(prev);
      return target;
    });
    setNavDir(direction);
    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      setPrevPage(null);
      setNavDir(null);
    }, NAV_DURATION);

    if (direction === 'push') {
      pageStackRef.current = [...pageStackRef.current, target];
      if (!isPopstateNavRef.current) {
        history.pushState({ page: target }, '');
      }
    } else if (direction === 'pop') {
      pageStackRef.current = pageStackRef.current.slice(0, -1);
      if (!isPopstateNavRef.current) {
        history.back();
      }
    }
  }, []);

  useEffect(() => () => clearTimeout(timerRef.current), []);

  // Open external links in a new tab from anywhere in the app.
  useEffect(() => {
    const handleLinkClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      const link = target.closest('a') as HTMLAnchorElement | null;
      
      if (link && link.href) {
        const href = link.href;
        // Treat all http(s) links as external for the mobile web shell.
        if (href.startsWith('http://') || href.startsWith('https://')) {
          e.preventDefault();
          e.stopPropagation();
          window.open(href, '_blank', 'noopener,noreferrer');
        }
      }
    };
    
    // Capture link clicks before nested content handles them.
    document.addEventListener('click', handleLinkClick, true);
    
    return () => {
      document.removeEventListener('click', handleLinkClick, true);
    };
  }, []);

  const handlePaired = useCallback(
    (client: RelayHttpClient, sessionMgr: RemoteSessionManager) => {
      clientRef.current = client;
      sessionMgrRef.current = sessionMgr;
      setSessionMgr(sessionMgr);
      pageStackRef.current = ['pairing', 'sessions'];
      history.pushState({ page: 'sessions' }, '');
      setPage('sessions');
    },
    [],
  );

  // Pop navigation handlers that can be called from both UI buttons and popstate
  const doPopFromChat = useCallback(() => {
    navigateTo('sessions', 'pop');
    setTimeout(() => setActiveSessionId(null), NAV_DURATION);
  }, [navigateTo]);

  const doPopFromWorkspace = useCallback(() => {
    navigateTo('sessions', 'pop');
  }, [navigateTo]);

  const doPopFromDevices = useCallback(() => {
    navigateTo('sessions', 'pop');
  }, [navigateTo]);

  useEffect(() => {
    const onPopState = () => {
      const stack = pageStackRef.current;
      const currentPage = stack[stack.length - 1];

      if (currentPage === 'pairing' || currentPage === 'sessions') {
        // At the root-level pages: re-push a history entry so the user
        // can't accidentally close the app with another back gesture.
        history.pushState({ page: currentPage }, '');
        return;
      }

      isPopstateNavRef.current = true;
      try {
        if (currentPage === 'chat') {
          doPopFromChat();
        } else if (currentPage === 'workspace') {
          doPopFromWorkspace();
        } else if (currentPage === 'devices') {
          doPopFromDevices();
        }
      } finally {
        isPopstateNavRef.current = false;
      }
    };

    window.addEventListener('popstate', onPopState);
    return () => window.removeEventListener('popstate', onPopState);
  }, [doPopFromChat, doPopFromWorkspace, doPopFromDevices]);

  const handleOpenWorkspace = useCallback(() => {
    navigateTo('workspace', 'push');
  }, [navigateTo]);

  const handleWorkspaceReady = useCallback(() => {
    navigateTo('sessions', 'pop');
  }, [navigateTo]);

  const handleSelectSession = useCallback((sessionId: string, sessionName?: string, isNew?: boolean) => {
    setActiveSessionId(sessionId);
    setActiveSessionName(sessionName || 'Session');
    setChatAutoFocus(!!isNew);
    navigateTo('chat', 'push');
  }, [navigateTo]);

  const handleBackToSessions = useCallback(() => {
    navigateTo('sessions', 'pop');
    setTimeout(() => setActiveSessionId(null), NAV_DURATION);
  }, [navigateTo]);

  const handleDisconnect = useCallback(() => {
    clientRef.current = null;
    sessionMgrRef.current = null;
    setSessionMgr(null);
    setActiveSessionId(null);
    setActiveSessionName('Session');
    setChatAutoFocus(false);
    setPrevPage(null);
    setNavDir(null);
    clearTimeout(timerRef.current);
    localStorage.removeItem('bitfun.mobile.user_id');
    useMobileStore.getState().resetConnectionState();
    pageStackRef.current = ['pairing'];
    setPage('pairing');
  }, []);

  const isAnimating = navDir !== null;
  const currentPage: Page = page;

  const shouldShow = (p: Page) => currentPage === p || (isAnimating && prevPage === p);

  return (
    <div className="mobile-app">
      {connectionHealth === 'unreachable' && page !== 'pairing' && (
        <div className="mobile-reconnect-banner" role="alert">
          <span className="mobile-reconnect-spinner" />
          <span>{t('sessions.reconnecting')}</span>
          <button type="button" onClick={handleDisconnect}>
            {t('sessions.repair')}
          </button>
        </div>
      )}
      {page === 'pairing' && <PairingPage onPaired={handlePaired} />}
      {shouldShow('workspace') && sessionMgrRef.current && (
        <div className={`nav-page ${getNavClass('workspace', currentPage, navDir, isAnimating)}`}>
          <WorkspacePage
            sessionMgr={sessionMgrRef.current}
            onReady={handleWorkspaceReady}
          />
        </div>
      )}
      {shouldShow('devices') && clientRef.current && (
        <div className={`nav-page ${getNavClass('devices', currentPage, navDir, isAnimating)}`}>
          <DevicesPage
            client={clientRef.current}
            onBack={doPopFromDevices}
          />
        </div>
      )}
      {shouldShow('sessions') && sessionMgrRef.current && (
        <div className={`nav-page ${getNavClass('sessions', currentPage, navDir, isAnimating)}`}>
          <SessionListPage
            sessionMgr={sessionMgrRef.current}
            onSelectSession={handleSelectSession}
            onOpenWorkspace={handleOpenWorkspace}
            onDisconnect={handleDisconnect}
            onOpenDevices={() => navigateTo('devices', 'push')}
          />
        </div>
      )}
      {shouldShow('chat') && sessionMgrRef.current && activeSessionId && (
        <div className={`nav-page ${getNavClass('chat', currentPage, navDir, isAnimating)}`}>
          <Suspense fallback={<div className="spinner" aria-hidden="true" />}>
            <ChatPage
              sessionMgr={sessionMgrRef.current}
              sessionId={activeSessionId}
              sessionName={activeSessionName}
              onBack={handleBackToSessions}
              autoFocus={chatAutoFocus}
            />
          </Suspense>
        </div>
      )}
    </div>
  );
};

const App: React.FC = () => (
  <ThemeProvider>
    <ErrorBoundary>
      <I18nProvider>
        <AppContent />
      </I18nProvider>
    </ErrorBoundary>
  </ThemeProvider>
);

export default App;
