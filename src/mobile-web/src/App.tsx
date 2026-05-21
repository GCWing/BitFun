import React, { useState, useCallback, useRef, useEffect } from 'react';
import PairingPage from './pages/PairingPage';
import WorkspacePage from './pages/WorkspacePage';
import SessionListPage from './pages/SessionListPage';
import ChatPage from './pages/ChatPage';
import { I18nProvider } from './i18n';
import { RelayHttpClient } from './services/RelayHttpClient';
import { RemoteSessionManager } from './services/RemoteSessionManager';
import { useMobileStore } from './services/store';
import type { ChatMessage } from './services/RemoteSessionManager';
import { ThemeProvider } from './theme';
import './styles/index.scss';

// Dev mode: skip pairing when URL contains ?dev. Creates mock services and seeds
// the store with sample data so UI features can be developed without a desktop instance.
function isDevMode(): boolean {
  return new URLSearchParams(window.location.search).has('dev');
}

function createMockMessages(): ChatMessage[] {
  const msgs: ChatMessage[] = [];
  for (let i = 1; i <= 20; i++) {
    msgs.push({
      id: `mock-user-${i}`,
      role: 'user',
      content: `这是第 ${i} 条用户消息。What does the git status command do?`,
      timestamp: new Date(Date.now() - (20 - i) * 60000).toISOString(),
    });
    msgs.push({
      id: `mock-assistant-${i}`,
      role: 'assistant',
      content: `## 回答 ${i}\n\n\`git status\` 显示工作目录和暂存区的状态。它告诉你哪些文件被修改了、哪些文件被暂存了、哪些文件没有被 Git 跟踪。\n\n### 常见用法\n\n\`\`\`bash\ngit status\n\`\`\`\n\n\`\`\`bash\ngit status -s  # 简短输出\n\`\`\`\n\n这是一个非常有用的命令，用来了解当前仓库的状态。`,
      timestamp: new Date(Date.now() - (20 - i) * 60000 + 2000).toISOString(),
      thinking: i % 3 === 0 ? '用户想了解 git status 命令。这是一个基础的 Git 问题，我应该给出清晰简洁的解释并附带示例。' : undefined,
      items: i % 5 === 0 ? [
        { type: 'tool' as const, tool: { id: 'tool-1', name: 'Bash', tool_input: { command: 'git status' }, status: 'completed' as const, input_preview: 'On branch main\nnothing to commit, working tree clean', duration_ms: 150 } },
      ] : undefined,
    });
  }
  return msgs;
}

type Page = 'pairing' | 'workspace' | 'sessions' | 'chat';
type NavDirection = 'push' | 'pop' | null;

const NAV_DURATION = 300;

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
  const devMode = isDevMode();
  const [page, setPage] = useState<Page>(devMode ? 'chat' : 'pairing');
  const [activeSessionId, setActiveSessionId] = useState<string | null>(devMode ? 'mock-session-1' : null);
  const [activeSessionName, setActiveSessionName] = useState<string>('Dev Session');
  const [chatAutoFocus, setChatAutoFocus] = useState(false);
  const clientRef = useRef<RelayHttpClient | null>(null);
  const sessionMgrRef = useRef<RemoteSessionManager | null>(null);
  const [devReady, setDevReady] = useState(false);

  // Dev mode: seed mock data on mount
  useEffect(() => {
    if (!devMode) return;
    const store = useMobileStore.getState();
    store.setSessions([
      { session_id: 'mock-session-1', name: 'Dev 会话 1', agent_type: 'code', created_at: new Date().toISOString(), updated_at: new Date().toISOString(), message_count: 40 },
      { session_id: 'mock-session-2', name: 'Dev 会话 2', agent_type: 'code', created_at: new Date(Date.now() - 3600000).toISOString(), updated_at: new Date(Date.now() - 3600000).toISOString(), message_count: 5 },
      { session_id: 'mock-session-3', name: 'Dev Cowork', agent_type: 'cowork', created_at: new Date(Date.now() - 7200000).toISOString(), updated_at: new Date(Date.now() - 7200000).toISOString(), message_count: 12 },
    ]);
    store.setMessages('mock-session-1', createMockMessages());
    store.setCurrentWorkspace({
      has_workspace: true,
      path: '/home/dev/sample-project',
      project_name: 'sample-project',
      git_branch: 'feat/dev-branch',
      workspace_kind: 'normal',
    });
    store.setPairedDisplayMode('pro');

    // Minimal mock client & session manager for UI interaction
    const mockCatalog: any = {
      models: [
        { id: 'auto', name: 'Auto', provider: 'auto', base_url: '', model_name: 'auto', enabled: true, capabilities: [] },
        { id: 'primary', name: 'Primary Model', provider: 'openai', base_url: '', model_name: 'gpt-4o', enabled: true, capabilities: ['thinking'] },
        { id: 'fast', name: 'Fast Model', provider: 'openai', base_url: '', model_name: 'gpt-4o-mini', enabled: true, capabilities: [] },
      ],
      default_models: { auto: 'auto', primary: 'primary', fast: 'fast' },
      session_model_id: 'auto',
    };
    const mockClient = {
      pair: async () => ({}),
      sendCommand: async (cmd: any) => {
        if (cmd.cmd === 'get_model_catalog') return { catalog: mockCatalog };
        if (cmd.cmd === 'set_session_model') return { model_id: cmd.model_id || 'auto' };
        if (cmd.cmd === 'get_session_messages') {
          const existing = useMobileStore.getState().getMessages(cmd.session_id);
          return { messages: existing, has_more: false };
        }
        return {};
      },
      get baseUrl() { return ''; },
      get room() { return 'dev'; },
    } as unknown as RelayHttpClient;
    const mockSessionMgr = new RemoteSessionManager(mockClient);
    clientRef.current = mockClient;
    sessionMgrRef.current = mockSessionMgr;
    setDevReady(true);
  }, [devMode]);

  const [navDir, setNavDir] = useState<NavDirection>(null);
  const [prevPage, setPrevPage] = useState<Page | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  // Track the page stack for browser history integration.
  // When user triggers browser back (phone back button / edge swipe),
  // we intercept popstate and perform in-app navigation instead.
  const pageStackRef = useRef<Page[]>(devMode ? ['chat'] : ['pairing']);
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

  // 全局链接点击处理 - 确保所有外部链接在新标签页打开
  useEffect(() => {
    const handleLinkClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      const link = target.closest('a') as HTMLAnchorElement | null;
      
      if (link && link.href) {
        const href = link.href;
        // 检查是否是外部链接 (http/https 且不是当前域名)
        if (href.startsWith('http://') || href.startsWith('https://')) {
          e.preventDefault();
          e.stopPropagation();
          window.open(href, '_blank', 'noopener,noreferrer');
        }
      }
    };
    
    // 添加全局点击监听
    document.addEventListener('click', handleLinkClick, true);
    
    return () => {
      document.removeEventListener('click', handleLinkClick, true);
    };
  }, []);

  const handlePaired = useCallback(
    (client: RelayHttpClient, sessionMgr: RemoteSessionManager) => {
      clientRef.current = client;
      sessionMgrRef.current = sessionMgr;
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
        }
      } finally {
        isPopstateNavRef.current = false;
      }
    };

    window.addEventListener('popstate', onPopState);
    return () => window.removeEventListener('popstate', onPopState);
  }, [doPopFromChat, doPopFromWorkspace]);

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

  const isAnimating = navDir !== null;
  const currentPage: Page = page;

  const shouldShow = (p: Page) => currentPage === p || (isAnimating && prevPage === p);
  const hasSessionMgr = devMode ? devReady : !!sessionMgrRef.current;

  return (
    <div className="mobile-app">
      {page === 'pairing' && !devMode && <PairingPage onPaired={handlePaired} />}
      {shouldShow('workspace') && hasSessionMgr && sessionMgrRef.current && (
        <div className={`nav-page ${getNavClass('workspace', currentPage, navDir, isAnimating)}`}>
          <WorkspacePage
            sessionMgr={sessionMgrRef.current}
            onReady={handleWorkspaceReady}
          />
        </div>
      )}
      {shouldShow('sessions') && hasSessionMgr && sessionMgrRef.current && (
        <div className={`nav-page ${getNavClass('sessions', currentPage, navDir, isAnimating)}`}>
          <SessionListPage
            sessionMgr={sessionMgrRef.current}
            onSelectSession={handleSelectSession}
            onOpenWorkspace={handleOpenWorkspace}
          />
        </div>
      )}
      {shouldShow('chat') && hasSessionMgr && sessionMgrRef.current && activeSessionId && (
        <div className={`nav-page ${getNavClass('chat', currentPage, navDir, isAnimating)}`}>
          <ChatPage
            sessionMgr={sessionMgrRef.current}
            sessionId={activeSessionId}
            sessionName={activeSessionName}
            onBack={handleBackToSessions}
            autoFocus={chatAutoFocus}
          />
        </div>
      )}
    </div>
  );
};

const App: React.FC = () => (
  <ThemeProvider>
    <I18nProvider>
      <AppContent />
    </I18nProvider>
  </ThemeProvider>
);

export default App;
