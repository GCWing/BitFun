import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const sessionListSource = readFileSync(
  new URL('../../../../../mobile-web/src/pages/SessionListPage.tsx', import.meta.url),
  'utf8',
);
const chatSource = readFileSync(
  new URL('../../../../../mobile-web/src/pages/ChatPage.tsx', import.meta.url),
  'utf8',
);
const devicesSource = readFileSync(
  new URL('../../../../../mobile-web/src/pages/DevicesPage.tsx', import.meta.url),
  'utf8',
);
const pairingSource = readFileSync(
  new URL('../../../../../mobile-web/src/pages/PairingPage.tsx', import.meta.url),
  'utf8',
);

describe('mobile control-target UI ownership contracts', () => {
  it('rebinds SessionList by target epoch and fences multi-step publications', () => {
    expect(sessionListSource).toContain(
      'sessionListOwnerRef.current.epoch !== controlTargetEpoch',
    );
    expect(sessionListSource).toContain('owner.epoch !== renderedEpoch');
    expect(sessionListSource).toContain('return renderedEpoch;');
    expect(sessionListSource).toContain('const targetEpoch = captureSessionListEpoch();');
    expect(sessionListSource).toContain('!isSessionListCurrent(targetEpoch)');
    expect(sessionListSource).toContain('clearLongPressTimer();');
    expect(sessionListSource).toContain('setShowWorkspacePicker(false);');
    expect(sessionListSource).toContain('setDeleteConfirmTarget(null);');
    expect(sessionListSource).toContain("setSearchQuery('');");
    expect(sessionListSource).toContain("setDisplayMode('pro');");
    expect(sessionListSource).toContain(
      'const [targetInitializing, setTargetInitializing] = useState(true);',
    );
    expect(sessionListSource).toContain('targetInitializingRef.current = true;');
    expect(sessionListSource).toContain('targetInitializingRef.current = false;');
    expect(sessionListSource).toContain(
      'if (creating || targetInitializingRef.current) return;',
    );
    expect(sessionListSource).toContain('disabled={creating || targetInitializing}');
    expect(sessionListSource).toMatch(
      /className="session-list__search-input"[\s\S]*?disabled=\{targetInitializing\}/,
    );
    expect(sessionListSource).not.toMatch(
      /className="session-list__search-input"[\s\S]*?disabled=\{loading\}/,
    );
    expect(sessionListSource).toContain('if (loading || loadingMore || !hasMore) return;');

    const firstPageOwner = sessionListSource.slice(
      sessionListSource.indexOf('const loadFirstPage = useCallback'),
      sessionListSource.indexOf('// Load workspace list for Pro mode picker'),
    );
    const refreshOwner = sessionListSource.slice(
      sessionListSource.indexOf('const refreshData = useCallback'),
      sessionListSource.indexOf('const poll = setInterval(refreshData'),
    );
    expect(firstPageOwner).toContain('setLoadingMore(false);');
    expect(refreshOwner).toContain('const requestSeq = ++listRequestSeqRef.current;');
    expect(refreshOwner).toContain('setLoading(false);');
    expect(refreshOwner).toContain('setLoadingMore(false);');
    expect(sessionListSource).toContain('useControlTargetEpoch(sessionMgr)');
    expect(sessionListSource).toContain('committedSessionListTargetRef');
    expect(sessionListSource).not.toContain(
      'sessionMgr.onControlTargetChange(invalidateRequests)',
    );
  });

  it('makes Chat StrictMode setup replay-safe and prevents orphan pollers', () => {
    const initEffect = chatSource.slice(
      chatSource.indexOf('const chatInitSeqRef = useRef(0);'),
      chatSource.indexOf('const prevMsgCountRef'),
    );

    expect(chatSource).toContain('committedChatTargetRef');
    expect(chatSource).toContain('useLayoutEffect(() => {');
    expect(chatSource).toContain('owner.active = false;');
    expect(chatSource).toContain('useControlTargetEpoch(sessionMgr)');
    expect(chatSource).not.toContain('sessionMgr.onControlTargetChange(() => {');
    expect(initEffect).toContain('const initSeq = ++chatInitSeqRef.current;');
    expect(initEffect).toContain('chatInitSeqRef.current === initSeq');
    expect(initEffect).toContain('cancelled = true;');
    expect(initEffect).toContain('if (!isInitCurrent()) return;');
  });

  it('fences a device probe and pairing name lookup to their original owners', () => {
    expect(devicesSource).toContain('client.delegatedAccountEpoch === accountEpoch');
    expect(devicesSource).toContain('client.controlTargetEpoch === expectedTargetEpoch');
    expect(devicesSource).toContain('expectedTargetEpoch = client.controlTargetEpoch;');

    expect(pairingSource).toContain('const target = client.getControlTargetSnapshot();');
    expect(pairingSource).toContain('!client.isControlTargetCurrent(target)');
    expect(pairingSource).toContain('client.pairedDeviceId !== homeDeviceId');
  });

  it('bootstraps pairing auto-reconnect once without resetting to a stuck spinner', () => {
    expect(pairingSource).toContain('attemptPairRef.current');
    expect(pairingSource).toContain('pairAttemptGenerationRef');
    expect(pairingSource).toContain('mount-once bootstrap');
    expect(pairingSource).toContain('eslint-disable-next-line react-hooks/exhaustive-deps -- mount-once bootstrap');
    // Regression: depending on attemptPair and unconditionally setting pairing
    // after a failed reconnect left the page spinning with no retry form.
    expect(pairingSource).not.toContain('autoReconnectAttemptedRef');
    expect(pairingSource).not.toMatch(
      /setConnectionStatus\(shouldAutoReconnect \? 'pairing' : 'idle'\)/,
    );
  });
});
