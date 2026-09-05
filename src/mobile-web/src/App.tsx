import React, { Suspense, lazy, useState, useCallback, useRef, useEffect } from 'react';
import { MobileBanner, MobileButton, MobileScrim, MobileStatus } from '@openbitfun/ui/mobile';
import PairingPage from './pages/PairingPage';
import WorkspacePage from './pages/WorkspacePage';
import SessionListPage from './pages/SessionListPage';
import DevicesPage from './pages/DevicesPage';
import { ErrorBoundary } from './components/ErrorBoundary';
import { I18nProvider, useI18n } from './i18n';
import { RelayHttpClient } from './services/RelayHttpClient';
import {
  RemoteSessionManager,
} from './services/RemoteSessionManager';
import { reconcileDelegatedAccountOwner } from './services/delegatedAccountOwner';
import { ThemeProvider } from './theme';
import { useConnectionHealth } from './hooks/useConnectionHealth';
import { useMobileViewport } from './hooks/useMobileViewport';
import { useWideLayout } from './hooks/useWideLayout';
import { useMobileStore } from './services/store';
import RemoteHomePanel from './components/RemoteHomePanel';
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
  useMobileViewport();
  const { t } = useI18n();
  const [page, setPage] = useState<Page>('pairing');
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeSessionName, setActiveSessionName] = useState<string>('Session');
  const [activeSessionAgentType, setActiveSessionAgentType] = useState('agentic');
  const [chatAutoFocus, setChatAutoFocus] = useState(false);
  const [compactSidebarOpen, setCompactSidebarOpen] = useState(false);
  const isWideLayout = useWideLayout();
  const connectionHealth = useMobileStore((state) => state.connectionHealth);
  const clientRef = useRef<RelayHttpClient | null>(null);
  const delegatedOwnerUnlistenRef = useRef<(() => void) | null>(null);
  const sessionMgrRef = useRef<RemoteSessionManager | null>(null);
  const [sessionMgr, setSessionMgr] = useState<RemoteSessionManager | null>(null);
  const [accountDirectoryOpen, setAccountDirectoryOpen] = useState(false);
  const [preferredDeviceId, setPreferredDeviceId] = useState<string | undefined>();

  // An authenticated account without a selected desktop has nothing to ping.
  useConnectionHealth(accountDirectoryOpen ? null : sessionMgr);

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
    (client: RelayHttpClient, sessionMgr: RemoteSessionManager, preferredDeviceId?: string) => {
      const needsDevice = client.hasDelegatedIdentity && !client.isPaired && !client.pairedDeviceId;
      setAccountDirectoryOpen(needsDevice);
      setPreferredDeviceId(preferredDeviceId);
      delegatedOwnerUnlistenRef.current?.();
      clientRef.current = client;
      delegatedOwnerUnlistenRef.current = client.onDelegatedAccountOwnerChange((change) => {
        if (clientRef.current !== client) return;
        const ownerScopedStateWasReset = reconcileDelegatedAccountOwner(change);
        if (!ownerScopedStateWasReset) return;

        // A detail page can retain local IDs in addition to Zustand state.
        // Return to the session root before any stale completion can render
        // data from the replacement account.
        clearTimeout(timerRef.current);
        setActiveSessionId(null);
        setActiveSessionName('Session');
        setActiveSessionAgentType('agentic');
        setChatAutoFocus(false);
        setPrevPage(null);
        setNavDir(null);
        pageStackRef.current = ['pairing', 'sessions'];
        history.replaceState({ page: 'sessions' }, '');
        setPage('sessions');
      }, { emitCurrent: true });
      sessionMgrRef.current = sessionMgr;
      setSessionMgr(sessionMgr);
      const landingPage = needsDevice ? 'devices' : 'sessions';
      pageStackRef.current = ['pairing', landingPage];
      history.pushState({ page: landingPage }, '');
      setPage(landingPage);
      setCompactSidebarOpen(false);
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

      if (accountDirectoryOpen || currentPage === 'pairing' || currentPage === 'sessions') {
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
  }, [accountDirectoryOpen, doPopFromChat, doPopFromWorkspace, doPopFromDevices]);

  const handleOpenWorkspace = useCallback(() => {
    navigateTo('workspace', 'push');
  }, [navigateTo]);

  const handleWorkspaceReady = useCallback(() => {
    navigateTo('sessions', 'pop');
  }, [navigateTo]);

  const handleSelectSession = useCallback((
    sessionId: string,
    sessionName?: string,
    isNew?: boolean,
    agentType = 'agentic',
  ) => {
    setActiveSessionId(sessionId);
    setActiveSessionName(sessionName || 'Session');
    setActiveSessionAgentType(agentType);
    setChatAutoFocus(!!isNew);
    setCompactSidebarOpen(false);
    if (isWideLayout) {
      clearTimeout(timerRef.current);
      setPrevPage(null);
      setNavDir(null);
      if (page !== 'chat') {
        pageStackRef.current = [...pageStackRef.current.filter((entry) => entry !== 'chat'), 'chat'];
        history.pushState({ page: 'chat' }, '');
      }
      setPage('chat');
      return;
    }
    navigateTo('chat', 'push');
  }, [isWideLayout, navigateTo, page]);

  const handleBackToSessions = useCallback(() => {
    navigateTo('sessions', 'pop');
    setTimeout(() => setActiveSessionId(null), NAV_DURATION);
  }, [navigateTo]);

  const handleControlTargetChanged = useCallback(() => {
    setAccountDirectoryOpen(false);
    clearTimeout(timerRef.current);
    setActiveSessionId(null);
    setActiveSessionName('Session');
    setActiveSessionAgentType('agentic');
    setChatAutoFocus(false);
    setPrevPage(null);
    setNavDir(null);
    pageStackRef.current = ['pairing', 'sessions'];
    history.replaceState({ page: 'sessions' }, '');
    setPage('sessions');
    setCompactSidebarOpen(true);
  }, []);

  const handleDisconnect = useCallback(() => {
    setAccountDirectoryOpen(false);
    setPreferredDeviceId(undefined);
    delegatedOwnerUnlistenRef.current?.();
    delegatedOwnerUnlistenRef.current = null;
    clientRef.current?.resetConnectionIdentity();
    clientRef.current = null;
    sessionMgrRef.current = null;
    setSessionMgr(null);
    setActiveSessionId(null);
    setActiveSessionName('Session');
    setActiveSessionAgentType('agentic');
    setChatAutoFocus(false);
    setCompactSidebarOpen(false);
    setPrevPage(null);
    setNavDir(null);
    clearTimeout(timerRef.current);
    localStorage.removeItem('openbitfun.mobile.user_id');
    useMobileStore.getState().resetConnectionState();
    pageStackRef.current = ['pairing'];
    setPage('pairing');
  }, []);

  useEffect(() => () => {
    delegatedOwnerUnlistenRef.current?.();
    delegatedOwnerUnlistenRef.current = null;
  }, []);

  const isAnimating = navDir !== null;
  const currentPage: Page = page;


  const renderSessionList = () => sessionMgrRef.current && (
    <SessionListPage
      sessionMgr={sessionMgrRef.current}
      client={clientRef.current ?? undefined}
      compact
      activeSessionId={activeSessionId}
      onSelectSession={handleSelectSession}
      onOpenWorkspace={handleOpenWorkspace}
      onDisconnect={handleDisconnect}
      onOpenDevices={() => navigateTo('devices', 'push')}
      onControlTargetChanged={handleControlTargetChanged}
    />
  );

  const renderDetailPage = () => {
    if (currentPage === 'workspace' && sessionMgrRef.current) {
      return (
        <WorkspacePage
          sessionMgr={sessionMgrRef.current}
          onReady={handleWorkspaceReady}
          onBack={doPopFromWorkspace}
        />
      );
    }
    if (currentPage === 'devices' && clientRef.current) {
      return <DevicesPage client={clientRef.current} onBack={doPopFromDevices} />;
    }
    if (currentPage === 'chat' && sessionMgrRef.current && activeSessionId) {
      return (
        <Suspense fallback={<MobileStatus loading title={t('workspace.loadingInfo')} />}>
          <ChatPage
            sessionMgr={sessionMgrRef.current}
            key={activeSessionId}
            sessionId={activeSessionId}
            sessionName={activeSessionName}
            agentType={activeSessionAgentType}
            onBack={isWideLayout ? handleBackToSessions : () => setCompactSidebarOpen(true)}
            autoFocus={chatAutoFocus}
            wideLayout={isWideLayout}
          />
        </Suspense>
      );
    }
    return (
      <RemoteHomePanel
        onOpenSidebar={isWideLayout ? undefined : () => setCompactSidebarOpen(true)}
      />
    );
  };

  return (
    <div className="mobile-app" data-layout={isWideLayout ? 'wide' : 'compact'}>
      {connectionHealth === 'unreachable' && page !== 'pairing' && (
        <MobileBanner
          action={<MobileButton appearance="plain" onClick={handleDisconnect} size="sm">{t('sessions.repair')}</MobileButton>}
          className="mobile-reconnect-banner"
          tone="danger"
        >
          <span className="mobile-reconnect-spinner" />
          <span>{t('sessions.reconnecting')}</span>
        </MobileBanner>
      )}
      {page === 'pairing' && <PairingPage onPaired={handlePaired} />}
      {accountDirectoryOpen && clientRef.current && (
        <DevicesPage
          client={clientRef.current}
          accountLanding
          preferredDeviceId={preferredDeviceId}
          onBack={handleDisconnect}
          onDeviceSelected={handleControlTargetChanged}
        />
      )}
      {!accountDirectoryOpen && page !== 'pairing' && sessionMgrRef.current && (
        <div className={isWideLayout ? 'remote-shell remote-shell--wide' : `nav-page compact-remote-shell${compactSidebarOpen ? ' is-sidebar-open' : ''}`}>
          <aside
            className={isWideLayout ? 'remote-shell__master' : 'compact-remote-shell__sidebar'}
            aria-label={t('sessions.sessionHistory')}
            aria-hidden={!isWideLayout && !compactSidebarOpen || undefined}
            ref={(node) => { if (node) node.inert = !isWideLayout && !compactSidebarOpen; }}
          >
            {renderSessionList()}
          </aside>
          <MobileScrim
            className="compact-remote-shell__scrim"
            hidden={isWideLayout || !compactSidebarOpen}
            aria-label={t('common.close')}
            onClick={() => setCompactSidebarOpen(false)}
          />
          <section className={isWideLayout ? 'remote-shell__detail' : `compact-remote-shell__main ${getNavClass(currentPage, currentPage, navDir, isAnimating && currentPage !== prevPage)}`}>
            {renderDetailPage()}
          </section>
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
