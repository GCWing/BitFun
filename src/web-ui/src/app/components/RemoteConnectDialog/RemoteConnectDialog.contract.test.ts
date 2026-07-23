import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const dialogSource = readFileSync(
  new URL('./RemoteConnectDialog.tsx', import.meta.url),
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
    expect(dialogSource).toContain('isOpen={isOpen && hasAgreedDisclaimer}');
    expect(dialogSource).toContain('isOpen={isOpen && (disclaimerIsGate || showDisclaimer)}');
  });

  it('binds tabs to accessible tab panels', () => {
    expect(dialogSource).toContain('aria-controls="remote-connect-panel-account"');
    expect(dialogSource).toContain('id="remote-connect-network-tabpanel"');
    expect(dialogSource).toContain('id="remote-connect-bot-tabpanel"');
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
    expect(dialogSource).toContain('handleDisconnectRelay,\n            accountUsername,');
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
    const finalizeFlow = accountPanelSource.slice(
      accountPanelSource.indexOf('const finalizeAndSync'),
      accountPanelSource.indexOf('const handleConfirmOverwrite'),
    );
    expect(finalizeFlow.match(/accountFinalizeLogin\(pendingLoginId\)/g)).toHaveLength(2);
    expect(finalizeFlow).toContain('pending login finalize response was ambiguous; retrying');
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

  it('restores an existing relay pairing as cancellable in-progress UI', () => {
    const restoreFlow = dialogSource.slice(
      dialogSource.indexOf('// On dialog open: check if a connection'),
      dialogSource.indexOf("activeGroup !== 'network'"),
    );
    expect(restoreFlow).toContain("pendingOwnerRef.current = 'network'");
    expect(restoreFlow).toContain("setConnectionOwner('network')");
    expect(restoreFlow).toContain('setConnectionResult({');
    expect(restoreFlow).toContain('qr_url: null');
    expect(restoreFlow).toContain("startPolling('relay')");
  });
});
