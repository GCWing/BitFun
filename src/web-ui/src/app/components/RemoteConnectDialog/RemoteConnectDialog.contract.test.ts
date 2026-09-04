import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const dialogSource = readFileSync(
  new URL('./RemoteConnectDialog.tsx', import.meta.url),
  'utf8',
);
const dialogStyleSource = readFileSync(
  new URL('./RemoteConnectDialog.scss', import.meta.url),
  'utf8',
);
const chatAppBrandIconSource = readFileSync(
  new URL('./ChatAppBrandIcon.tsx', import.meta.url),
  'utf8',
);
const deviceStatusControlSource = readFileSync(
  new URL('../NavPanel/components/DeviceStatusControl.tsx', import.meta.url),
  'utf8',
);
const navPanelStyleSource = readFileSync(
  new URL('../NavPanel/NavPanel.scss', import.meta.url),
  'utf8',
);
const accountPanelSource = readFileSync(
  new URL('./AccountPanel.tsx', import.meta.url),
  'utf8',
);
const remoteConnectApiSource = readFileSync(
  new URL('../../../infrastructure/api/service-api/RemoteConnectAPI.ts', import.meta.url),
  'utf8',
);
const accountLoginStateSource = readFileSync(
  new URL('../../../infrastructure/account/useAccountLoginState.ts', import.meta.url),
  'utf8',
);

describe('Remote Connect safety contracts', () => {
  it('gates the complete dialog surface behind disclaimer agreement', () => {
    expect(dialogSource).toContain('open={isOpen && hasAgreedDisclaimer}');
    expect(dialogSource).toContain('open={isOpen && (disclaimerIsGate || showDisclaimer)}');
  });

  it('presents one overview with account and account-free destinations', () => {
    const overview = dialogSource.slice(
      dialogSource.indexOf('const renderOverview ='),
      dialogSource.indexOf('const renderViewHeader'),
    );

    expect(overview).toContain('remote-connect-my-devices-title');
    expect(overview).toContain('remote-connect-access-title');
    expect(overview.match(/renderOverviewAction\(\{/g)).toHaveLength(3);
    expect(overview).toContain("view: 'account'");
    expect(overview).toContain("view: 'network'");
    expect(overview).toContain("view: 'bot'");
    expect(dialogSource).not.toContain('data-openbitfun-part="groupTab"');
    expect(dialogSource).not.toContain('remote-connect-group-');
  });

  it('keeps persistent connection context beside a single task surface', () => {
    expect(dialogSource).toContain('size="2xl"');
    expect(dialogSource).toContain('className="openbitfun-remote-connect-dialog"');
    expect(dialogSource).toContain('className="openbitfun-remote-connect-dialog__header"');
    expect(dialogSource).toContain('className="openbitfun-remote-connect-dialog__body"');
    expect(dialogSource).toContain('data-openbitfun-part="sidebar"');
    expect(dialogSource).toContain('data-openbitfun-part="sidebarBrand"');
    expect(dialogSource).toContain('data-openbitfun-part="main"');
    expect(dialogSource).toContain("t('remoteConnect.overviewIntro')");
  });

  it('keeps the dialog height stable while selected content scrolls inside it', () => {
    const desktopGeometry = dialogStyleSource.slice(
      dialogStyleSource.indexOf('.openbitfun-remote-connect-dialog {'),
      dialogStyleSource.indexOf('.openbitfun-remote-connect-dialog__header'),
    );

    expect(desktopGeometry).toContain('block-size: min(620px, calc(100vh - 2 * var(--openbitfun-overlay-dialog-viewport-gutter)))');
    expect(desktopGeometry).toContain('min-block-size: min(620px, calc(100vh - 2 * var(--openbitfun-overlay-dialog-viewport-gutter)))');
    expect(desktopGeometry).toContain('max-block-size: min(620px, calc(100vh - 2 * var(--openbitfun-overlay-dialog-viewport-gutter)))');
    expect(dialogStyleSource).toContain(".openbitfun-remote-connect [data-openbitfun-part='panel']");
    expect(dialogSource).toContain('<ScrollArea');
  });

  it('delegates accessible method and provider tabs to the design system', () => {
    expect(dialogSource).toContain('id="remote-connect-network-tabpanel"');
    expect(dialogSource).toContain('id="remote-connect-bot-tabpanel"');
    expect(dialogSource).toContain("panelId: 'remote-connect-network-tabpanel'");
    expect(dialogSource).toContain("panelId: 'remote-connect-bot-tabpanel'");
    expect(dialogSource.match(/<TabGroup/g)).toHaveLength(2);
    expect(dialogSource).not.toContain('handleTabArrowKey');
    expect(dialogSource).not.toContain('data-openbitfun-part="subtab"');
  });

  it('preserves all network methods and chat providers', () => {
    const methods = dialogSource.slice(
      dialogSource.indexOf('const NETWORK_TABS'),
      dialogSource.indexOf('const NGROK_SETUP_URL'),
    );

    expect(methods).toContain("id: 'lan'");
    expect(methods).toContain("id: 'openbitfun_server'");
    expect(methods).toContain("id: 'ngrok'");
    expect(methods).toContain("id: 'custom_server'");
    expect(methods).toContain("id: 'telegram'");
    expect(methods).toContain("id: 'feishu'");
    expect(methods).toContain("id: 'weixin'");
  });

  it('uses the real monochrome app marks for every chat provider', () => {
    const overviewBrandStyle = dialogStyleSource.slice(
      dialogStyleSource.indexOf('.openbitfun-remote-connect__chat-brand-item'),
      dialogStyleSource.indexOf(
        "[data-openbitfun-component='remote-connect-dialog'][data-openbitfun-part='overviewAction'][data-openbitfun-group='account']",
      ),
    );
    const identityBrandStyle = dialogStyleSource.slice(
      dialogStyleSource.indexOf('.openbitfun-remote-connect__bot-identity-icon'),
      dialogStyleSource.indexOf('.openbitfun-remote-connect__bot-identity-title'),
    );
    const connectedBrandStyle = dialogStyleSource.slice(
      dialogStyleSource.indexOf('.openbitfun-remote-connect__connected-app-icon'),
      dialogStyleSource.indexOf('.openbitfun-remote-connect__connected-app-copy'),
    );
    const footerMessageBrandStyle = navPanelStyleSource.slice(
      navPanelStyleSource.indexOf("&[data-openbitfun-device-kind='message-app'] {"),
      navPanelStyleSource.indexOf('.openbitfun-nav-panel__footer-device-status-attached-count'),
    );
    const overviewMessageBrandStart = navPanelStyleSource.indexOf(
      "&[data-openbitfun-device-kind='message-app'] .openbitfun-device-overview__device-icon {",
    );
    const overviewMessageBrandStyle = navPanelStyleSource.slice(
      overviewMessageBrandStart,
      navPanelStyleSource.indexOf('  strong {', overviewMessageBrandStart),
    );

    expect(dialogSource).toContain('<ChatAppBrandIcon app={botTab} size={28} />');
    expect(dialogSource).toContain('openbitfun-remote-connect__chat-brand-group');
    expect(dialogSource).toContain('<ChatAppBrandIcon app={brand} size={15} />');
    expect(chatAppBrandIconSource).toContain("app === 'telegram'");
    expect(chatAppBrandIconSource).toContain("app === 'feishu'");
    expect(chatAppBrandIconSource.match(/viewBox="0 0 24 24"/g)).toHaveLength(3);
    expect(chatAppBrandIconSource.match(/fill="currentColor"/g)).toHaveLength(5);
    expect(deviceStatusControlSource).toContain('chatAppBrandFromIdentity(identity)');
    expect(deviceStatusControlSource).toContain('<ChatAppBrandIcon app={chatApp} size={size} />');
    expect(overviewBrandStyle).toContain('border: 0');
    expect(overviewBrandStyle).toContain('background: transparent');
    expect(identityBrandStyle).not.toContain('background:');
    expect(connectedBrandStyle).not.toContain('background:');
    expect(footerMessageBrandStyle).toContain('border: 0');
    expect(footerMessageBrandStyle).toContain('background: transparent');
    expect(footerMessageBrandStyle).toContain('--openbitfun-color-content-primary');
    expect(overviewMessageBrandStyle).toContain('background: transparent');
    expect(overviewMessageBrandStyle).toContain('--openbitfun-color-content-primary');
    expect(dialogSource).not.toContain('<Send size={28} />');
    expect(dialogSource).not.toContain('<MessageSquareText size={28} />');
    expect(dialogSource).not.toContain('<MessagesSquare size={28} />');
  });

  it('keeps OpenBitFun Page out of the account and device lifecycle', () => {
    expect(accountPanelSource).not.toContain('pagesEntry');
    expect(accountPanelSource).not.toContain("openScene('pages')");
    expect(accountPanelSource).not.toContain('PanelsTopLeft');
  });

  it('does not issue an unconditional logout for a late 401 response', () => {
    const handler = accountPanelSource.slice(
      accountPanelSource.indexOf('const handleSessionExpired'),
      accountPanelSource.indexOf('const markRelayUnreachable'),
    );
    expect(handler).not.toContain('accountLogout');
    expect(handler).toContain('isAccountEpochCurrent(expectedEpoch)');
  });

  it('binds presence updates to the account epoch that created the listener', () => {
    const listener = accountPanelSource.slice(
      accountPanelSource.indexOf('// Subscribe only while a specific account epoch is active.'),
      accountPanelSource.indexOf('const validate'),
    );
    const invalidation = accountPanelSource.slice(
      accountPanelSource.indexOf('const invalidateAccountRequests'),
      accountPanelSource.indexOf('const isAccountEpochCurrent'),
    );

    expect(listener).toContain('const subscribedEpoch = activeAccountEpoch');
    expect(listener).toContain('isAccountEpochCurrent(subscribedEpoch)');
    expect(listener).not.toContain('isAccountEpochCurrent(accountEpochRef.current)');
    expect(invalidation).toContain('setActiveAccountEpoch(null)');
  });

  it('debounces transient device-list failures while device routing remains healthy', () => {
    const refreshFlow = accountPanelSource.slice(
      accountPanelSource.indexOf('const refreshDevices'),
      accountPanelSource.indexOf('const applyPresenceOnline'),
    );

    expect(refreshFlow).toContain('refreshInFlightRef.current?.epoch === epoch');
    expect(refreshFlow).toContain('deviceListFailureCountRef.current += 1');
    expect(refreshFlow).toContain('!deviceRoutingReadyRef.current');
    expect(refreshFlow).toContain('DEVICE_LIST_FAILURE_THRESHOLD');
    expect(refreshFlow.indexOf('!deviceRoutingReadyRef.current')).toBeLessThan(
      refreshFlow.indexOf('markRelayUnreachable()'),
    );
  });

  it('retries initial device routing without starting a duplicate sync-time connection', () => {
    const retryHelper = accountPanelSource.slice(
      accountPanelSource.indexOf('async function connectDevicesWithRetry'),
      accountPanelSource.indexOf('function parseRelayServer'),
    );
    const backgroundSync = accountPanelSource.slice(
      accountPanelSource.indexOf('const startBackgroundSync'),
      accountPanelSource.indexOf('const handleRetrySync'),
    );

    expect(retryHelper).toContain('DEVICE_CONNECT_MAX_ATTEMPTS');
    expect(retryHelper).toContain('isRelayUnreachable(error)');
    expect(backgroundSync).not.toContain('accountConnectDevices');
  });

  it('keeps recovering an initial device-routing failure without overlapping attempts', () => {
    const recoveryFlow = accountPanelSource.slice(
      accountPanelSource.indexOf('const attemptDeviceReconnect'),
      accountPanelSource.indexOf('/** Connect presence + load the device list'),
    );

    expect(recoveryFlow).toContain('deviceReconnectInFlightRef.current');
    expect(recoveryFlow).toContain('DEVICE_CONNECT_RECOVERY_INTERVAL_MS');
    expect(recoveryFlow).toContain('attemptDeviceReconnect(false)');
    expect(recoveryFlow).toContain('startDevicePolling()');
  });

  it('delegates transient retries without replaying the complete account sync workflow', () => {
    const backgroundSync = accountPanelSource.slice(
      accountPanelSource.indexOf('const startBackgroundSync'),
      accountPanelSource.indexOf('const handleRetrySync'),
    );

    expect(backgroundSync).toContain('AccountClient owns transient Relay retries');
    expect(backgroundSync).not.toContain('for (let attempt');
    expect(backgroundSync.match(/accountAutoSync/g)).toHaveLength(1);
  });

  it('binds overwrite finalize and cleanup to an opaque pending login id', () => {
    expect(accountPanelSource).toContain('pendingLoginIdRef.current = result.pending_login_id');
    expect(accountPanelSource).toContain('accountFinalizeLogin(pendingLoginId)');
    expect(accountPanelSource).toContain('accountCancelPendingLogin(pendingLoginId)');

    const overwriteCleanupStart = accountPanelSource.indexOf(
      '// Unmounting (dialog close or group switch)',
    );
    const overwriteCleanup = accountPanelSource.slice(
      overwriteCleanupStart,
      accountPanelSource.indexOf('remoteConnectAPI.getDeviceInfo()', overwriteCleanupStart),
    );
    expect(overwriteCleanup).toContain('cancelPendingLoginWithRetry(pendingLoginId)');
    expect(overwriteCleanup).not.toContain('accountLogout');
  });

  it('does not expose the account bearer token in the login result contract', () => {
    const loginResult = remoteConnectApiSource.slice(
      remoteConnectApiSource.indexOf('export interface AccountLoginResult'),
      remoteConnectApiSource.indexOf('export interface AccountHint'),
    );
    expect(loginResult).toContain('pending_login_id: string | null');
    expect(loginResult).not.toContain('token:');
  });

  it('uses verified usernames instead of opaque account ids in user-facing login states', () => {
    const connectedView = dialogSource.slice(
      dialogSource.indexOf('const renderConnectedView'),
      dialogSource.indexOf('const handleCopyPairingUrl'),
    );
    const performLogin = accountPanelSource.slice(
      accountPanelSource.indexOf('const performLogin'),
      accountPanelSource.indexOf('const handleLogin'),
    );

    expect(dialogSource).toContain('remoteConnectAPI.accountGetCredentialHint()');
    expect(dialogSource).toContain('setAccountUsername(hint?.username.trim() || null)');
    expect(connectedView).toContain("t('accountLogin.username')");
    expect(connectedView).not.toContain('connectedUserId');
    expect(dialogSource).toMatch(/handleDisconnectRelay,\s+accountUsername,/);
    expect(performLogin).toContain("loginSuccess', { user_id: user }");
    expect(performLogin).not.toContain("loginSuccess', { user_id: result.user_id }");
  });

  it('keeps transport failures distinct from a stale pending-owner response', () => {
    const cancelMethod = remoteConnectApiSource.slice(
      remoteConnectApiSource.indexOf('async accountCancelPendingLogin'),
      remoteConnectApiSource.indexOf('async accountStatus'),
    );
    expect(cancelMethod).toContain('throw e');
    expect(cancelMethod).not.toContain('return false');
  });

  it('does not reinterpret an account-status transport failure as logout', () => {
    const statusMethod = remoteConnectApiSource.slice(
      remoteConnectApiSource.indexOf('async accountStatus'),
      remoteConnectApiSource.indexOf('async accountGetCredentialHint'),
    );
    const accountPanelInitialization = accountPanelSource.slice(
      accountPanelSource.indexOf('remoteConnectAPI.accountStatus().then'),
      accountPanelSource.indexOf(
        'return () => {',
        accountPanelSource.indexOf('remoteConnectAPI.accountStatus().then'),
      ),
    );
    const sharedStateRefresh = accountLoginStateSource.slice(
      accountLoginStateSource.indexOf('const refresh = async () =>'),
      accountLoginStateSource.indexOf('void refresh();'),
    );

    expect(statusMethod).toContain('throw e');
    expect(statusMethod).not.toContain('logged_in: false');
    expect(accountPanelInitialization).toContain('}).catch((e) => {');
    expect(sharedStateRefresh).toContain("log.warn('Failed to refresh account login state', error)");
    expect(sharedStateRefresh.indexOf('return;')).toBeLessThan(
      sharedStateRefresh.indexOf('setState({ loggedIn: false'),
    );
  });

  it('does not discard a pending owner when conditional cleanup transport fails', () => {
    const cancelFlow = accountPanelSource.slice(
      accountPanelSource.indexOf('const handleCancelOverwrite'),
      accountPanelSource.indexOf('const handleLogout'),
    );
    expect(cancelFlow).toContain('await cancelPendingLoginWithRetry(pendingLoginId)');
    expect(cancelFlow.indexOf('pendingLoginIdRef.current = null')).toBeGreaterThan(
      cancelFlow.indexOf('await cancelPendingLoginWithRetry(pendingLoginId)'),
    );
    expect(cancelFlow).toContain("log.warn('pending login cancel failed', e)");
    expect(cancelFlow).toContain('return;');
  });

  it('retries an ambiguous finalize response with the same opaque owner', () => {
    const retryHelper = accountPanelSource.slice(
      accountPanelSource.indexOf('async function finalizePendingLoginWithRetry'),
      accountPanelSource.indexOf('/** Quota / payload-limit failures'),
    );
    expect(retryHelper).toContain('ACCOUNT_TRANSITION_MAX_ATTEMPTS');
    expect(retryHelper).toContain('accountFinalizeLogin(pendingLoginId)');
    expect(retryHelper).toContain('was ambiguous; retrying');
  });

  it('invalidates the prior background sync before starting a replacement login', () => {
    const performLogin = accountPanelSource.slice(
      accountPanelSource.indexOf('const performLogin'),
      accountPanelSource.indexOf('const handleLogin'),
    );
    expect(performLogin.indexOf('syncInFlightRef.current = false')).toBeLessThan(
      performLogin.indexOf('remoteConnectAPI.accountLogin'),
    );
    expect(performLogin.indexOf('clearSync()')).toBeLessThan(
      performLogin.indexOf('remoteConnectAPI.accountLogin'),
    );
  });

  it('fences Weixin poll rejection cleanup to the operation that owns the UI', () => {
    const pollEffect = dialogSource.slice(
      dialogSource.indexOf('// WeChat QR login: poll iLink'),
      dialogSource.indexOf('// ── Connection handlers'),
    );
    const rejectionCleanup = pollEffect.slice(
      pollEffect.lastIndexOf('} catch (e: unknown) {'),
      pollEffect.lastIndexOf('return;'),
    );

    expect(rejectionCleanup).toContain('updateIfOperationCurrent(isCurrent, () => {');
    expect(rejectionCleanup).toContain('setWeixinQrSessionKey(null)');
    expect(rejectionCleanup).toContain('setWeixinQrImageUrl(null)');
    expect(rejectionCleanup).toContain('setWeixinAwaitingPhoneConfirm(false)');
  });

  it('auto-starts Weixin after QR login without exposing a redundant Connect action', () => {
    const botContent = dialogSource.slice(
      dialogSource.indexOf('const renderBotContent'),
      dialogSource.indexOf('// ── Layout'),
    );

    expect(botContent).toContain("botTab !== 'weixin'");
    expect(botContent).not.toContain('botWeixinLinked');
    expect(dialogSource).toContain("t('remoteConnect.botWeixinRestriction')");
    expect(dialogSource).toContain('prepareAndStartWeixinBotFromQr');
  });

  it('restores an existing relay pairing as cancellable in-progress UI', () => {
    const restoreFlow = dialogSource.slice(
      dialogSource.indexOf('// On dialog open: check if a connection'),
      dialogSource.indexOf("activeView !== 'network'"),
    );
    expect(restoreFlow).toContain("pendingOwnerRef.current = 'network'");
    expect(restoreFlow).toContain("setConnectionOwner('network')");
    expect(restoreFlow).toContain('setConnectionResult({');
    expect(restoreFlow).toContain('qr_url: null');
    expect(restoreFlow).toContain("startPolling('relay')");
  });

  it('restores connected method status without hijacking the overview', () => {
    const applyStatus = dialogSource.slice(
      dialogSource.indexOf('const applyStatus'),
      dialogSource.indexOf('const startPolling'),
    );
    const restoreFlow = dialogSource.slice(
      dialogSource.indexOf('const checkExisting'),
      dialogSource.indexOf("activeView !== 'network'"),
    );
    const connectedRestore = restoreFlow.slice(
      restoreFlow.indexOf('applyStatus(s)'),
      restoreFlow.indexOf("if (['waiting_for_scan'"),
    );

    expect(applyStatus).toContain("remotePairingStateName(nextStatus.pairing_state) === 'connected'");
    expect(applyStatus).toContain('setNetworkTab(connectedTab)');
    expect(applyStatus).toContain('setBotTab(connectedBot)');
    expect(dialogSource).toContain("useState<ActiveView>(initialGroup ?? 'overview')");
    expect(connectedRestore).not.toContain('setActiveView');
  });
});
