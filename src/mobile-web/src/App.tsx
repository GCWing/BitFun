import React, { useState, useCallback, useRef } from 'react';
import PairingPage from './pages/PairingPage';
import WorkspacePage from './pages/WorkspacePage';
import SessionListPage from './pages/SessionListPage';
import ChatPage from './pages/ChatPage';
import { RelayHttpClient } from './services/RelayHttpClient';
import { RemoteSessionManager } from './services/RemoteSessionManager';
import { ThemeProvider } from './theme';
import './styles/index.scss';

type Page = 'pairing' | 'workspace' | 'sessions' | 'chat';

const AppContent: React.FC = () => {
  const [page, setPage] = useState<Page>('pairing');
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeSessionName, setActiveSessionName] = useState<string>('Session');
  const clientRef = useRef<RelayHttpClient | null>(null);
  const sessionMgrRef = useRef<RemoteSessionManager | null>(null);

  const handlePaired = useCallback(
    (client: RelayHttpClient, sessionMgr: RemoteSessionManager) => {
      clientRef.current = client;
      sessionMgrRef.current = sessionMgr;
      setPage('sessions');
    },
    [],
  );

  const handleOpenWorkspace = useCallback(() => {
    setPage('workspace');
  }, []);

  const handleWorkspaceReady = useCallback(() => {
    setPage('sessions');
  }, []);

  const handleSelectSession = useCallback((sessionId: string, sessionName?: string) => {
    setActiveSessionId(sessionId);
    setActiveSessionName(sessionName || 'Session');
    setPage('chat');
  }, []);

  const handleBackToSessions = useCallback(() => {
    setActiveSessionId(null);
    setPage('sessions');
  }, []);

  return (
    <div className="mobile-app">
      {page === 'pairing' && <PairingPage onPaired={handlePaired} />}
      {page === 'workspace' && sessionMgrRef.current && (
        <WorkspacePage
          sessionMgr={sessionMgrRef.current}
          onReady={handleWorkspaceReady}
        />
      )}
      {page === 'sessions' && sessionMgrRef.current && (
        <SessionListPage
          sessionMgr={sessionMgrRef.current}
          onSelectSession={handleSelectSession}
          onOpenWorkspace={handleOpenWorkspace}
        />
      )}
      {page === 'chat' && sessionMgrRef.current && activeSessionId && (
        <ChatPage
          sessionMgr={sessionMgrRef.current}
          sessionId={activeSessionId}
          sessionName={activeSessionName}
          onBack={handleBackToSessions}
        />
      )}
    </div>
  );
};

const App: React.FC = () => (
  <ThemeProvider>
    <AppContent />
  </ThemeProvider>
);

export default App;
