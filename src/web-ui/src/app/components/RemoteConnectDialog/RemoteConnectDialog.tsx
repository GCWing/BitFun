/**
 * Remote Connect dialog with three groups:
 *   - Network (LAN / Ngrok / BitFun Server / Custom Server) – mutually exclusive
 *   - IM Bot (Telegram / Feishu / WeChat) – mutually exclusive
 *   - Account / My BitFun (login, cloud sync, peer device control, Pages entry)
 * Network and Bot require an open workspace and can be active simultaneously;
 * the Account group works without a workspace.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { useI18n } from '@/infrastructure/i18n';
import { getLocaleFallbackChain, type LocaleId } from '@/infrastructure/i18n/presets';
import { Modal, Badge, Input, Select, Tooltip } from '@/component-library';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useAccountLoginState } from '@/infrastructure/account/useAccountLoginState';
import { useNotification } from '@/shared/notification-system';
import { copyTextToClipboard } from '@/shared/utils/textSelection';
import { AccountPanel } from './AccountPanel';
import {
  remoteConnectAPI,
  type ConnectionResult,
  type RemoteConnectStatus,
  type LanNetworkInterface,
} from '@/infrastructure/api/service-api/RemoteConnectAPI';
import {
  RemoteConnectDisclaimerContent,
} from './RemoteConnectDisclaimer';
import {
  getRemoteConnectDisclaimerAgreed,
  setRemoteConnectDisclaimerAgreed,
} from './remoteConnectDisclaimerStorage';
import { RelayDeployWizard } from '@/features/relay-deploy';
import type { RelayDeployResult } from '@/features/relay-deploy';
import {
  stopAfterPendingStart,
  updateIfOperationCurrent,
} from './remoteConnectOperationCleanup';
import './RemoteConnectDialog.scss';

// ── Types ────────────────────────────────────────────────────────────

type ActiveGroup = 'network' | 'bot' | 'account';
type ConnectionOwner = Exclude<ActiveGroup, 'account'>;
type NetworkTab = 'lan' | 'ngrok' | 'bitfun_server' | 'custom_server';
type BotTab = 'telegram' | 'feishu' | 'weixin';

/**
 * iLink `qrcode_img_content` is the string to encode in a QR (OpenClaw passes it to
 * `qrcode-terminal.generate`), not necessarily an `<img src>` raster URL. Only treat
 * as raster when it is clearly a data-URL or direct image link.
 */
function isWeixinRasterQrSrc(raw: string): boolean {
  const t = raw.trim();
  if (/^data:image\//i.test(t)) return true;
  if (
    /^https?:\/\//i.test(t)
    && /\.(png|jpe?g|gif|webp|svg)(\?|#|$)/i.test(t)
  ) {
    return true;
  }
  return false;
}

const NETWORK_TABS: { id: NetworkTab; labelKey: string }[] = [
  { id: 'lan', labelKey: 'shared:connectionMethods.lan' },
  { id: 'ngrok', labelKey: 'remoteConnect.tabNgrok' },
  { id: 'bitfun_server', labelKey: 'shared:connectionMethods.bitfunServer' },
  { id: 'custom_server', labelKey: 'remoteConnect.tabCustomServer' },
];

const BOT_TABS: { id: BotTab; label: string }[] = [
  { id: 'telegram', label: 'Telegram' },
  { id: 'feishu', label: '' }, // filled from i18n
  { id: 'weixin', label: '' },
];

const NGROK_SETUP_URL = 'https://dashboard.ngrok.com/get-started/setup';
const FEISHU_SETUP_GUIDE_URLS = {
  'zh-CN': 'https://github.com/GCWing/BitFun/blob/main/docs/remote-connect/feishu-bot-setup.zh-CN.md',
  'en-US': 'https://github.com/GCWing/BitFun/blob/main/docs/remote-connect/feishu-bot-setup.md',
} as const satisfies Partial<Record<LocaleId, string>>;

function pickLocalizedUrl(urls: Partial<Record<LocaleId, string>>, locale: LocaleId): string {
  for (const localeId of getLocaleFallbackChain(locale, true)) {
    const url = urls[localeId];
    if (url) return url;
  }

  return urls['en-US'] ?? Object.values(urls)[0] ?? '';
}

function parseRelayServer(value: string): URL | null {
  try {
    const url = new URL(value.trim());
    if (!['http:', 'https:'].includes(url.protocol)
      || !url.hostname
      || url.username
      || url.password
      || url.search
      || url.hash) {
      return null;
    }
    return url;
  } catch {
    return null;
  }
}

const methodToNetworkTab = (method: string | null | undefined): NetworkTab | null => {
  if (!method) return null;
  if (method.startsWith('Lan')) return 'lan';
  if (method.startsWith('Ngrok')) return 'ngrok';
  if (method.startsWith('BitfunServer')) return 'bitfun_server';
  if (method.startsWith('CustomServer')) return 'custom_server';
  return null;
};

const botInfoToBotTab = (info: string | null | undefined): BotTab | null => {
  if (!info) return null;
  if (info.startsWith('Telegram')) return 'telegram';
  if (info.startsWith('Feishu')) return 'feishu';
  if (info.startsWith('Weixin')) return 'weixin';
  return null;
};

// ── Component ────────────────────────────────────────────────────────

interface RemoteConnectDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /**
   * Group to show when no active connection is detected (e.g. the menu opens
   * the account group after a token expiry). Active relay/bot connections
   * still take precedence and switch to their own group.
   */
  initialGroup?: ActiveGroup;
}

export const RemoteConnectDialog: React.FC<RemoteConnectDialogProps> = ({
  isOpen,
  onClose,
  initialGroup,
}) => {
  const { t, currentLanguage } = useI18n('common');
  const { error: notifyError } = useNotification();
  const { hasWorkspace } = useCurrentWorkspace();
  const { loggedIn: accountLoggedIn } = useAccountLoginState();

  const [activeGroup, setActiveGroup] = useState<ActiveGroup>(
    initialGroup ?? (hasWorkspace ? 'network' : 'account'),
  );
  const [networkTab, setNetworkTab] = useState<NetworkTab>(NETWORK_TABS[0].id);
  const [botTab, setBotTab] = useState<BotTab>(BOT_TABS[0].id);

  const [connectionResult, setConnectionResult] = useState<ConnectionResult | null>(null);
  const [connectionOwner, setConnectionOwner] = useState<ConnectionOwner | null>(null);
  const [status, setStatus] = useState<RemoteConnectStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lanNetworkInfo, setLanNetworkInfo] = useState<{
    localIp: string;
    gatewayIp: string | null;
    availableIps: LanNetworkInterface[];
  } | null>(null);
  const [selectedLanIp, setSelectedLanIp] = useState<string>('');
  const [showDisclaimer, setShowDisclaimer] = useState(false);
  const [hasAgreedDisclaimer, setHasAgreedDisclaimer] = useState<boolean>(() => getRemoteConnectDisclaimerAgreed());
  const [botVerboseMode, setBotVerboseMode] = useState<boolean>(false);
  const [showRelayDeploy, setShowRelayDeploy] = useState(false);
  const [accountUsername, setAccountUsername] = useState<string | null>(null);

  const [qrCopied, setQrCopied] = useState(false);
  const [customUrl, setCustomUrl] = useState('');
  const [tgToken, setTgToken] = useState('');
  const [feishuAppId, setFeishuAppId] = useState('');
  const [feishuAppSecret, setFeishuAppSecret] = useState('');
  const [weixinIlinkToken, setWeixinIlinkToken] = useState('');
  const [weixinBaseUrl, setWeixinBaseUrl] = useState('');
  const [weixinBotAccountId, setWeixinBotAccountId] = useState('');
  const [weixinQrSessionKey, setWeixinQrSessionKey] = useState<string | null>(null);
  const [weixinQrImageUrl, setWeixinQrImageUrl] = useState<string | null>(null);
  const [weixinAwaitingPhoneConfirm, setWeixinAwaitingPhoneConfirm] = useState(false);

  const handleTabArrowKey = useCallback((event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    const tabList = event.currentTarget.closest('[role="tablist"]');
    if (!tabList) return;
    const tabs = Array.from(
      tabList.querySelectorAll<HTMLButtonElement>('[role="tab"]:not(:disabled)'),
    );
    const currentIndex = tabs.indexOf(event.currentTarget);
    if (currentIndex < 0 || tabs.length === 0) return;
    event.preventDefault();
    const nextIndex = event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? tabs.length - 1
        : (currentIndex + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
    tabs[nextIndex].focus();
    tabs[nextIndex].click();
  }, []);

  const formSnapshotRef = useRef({
    customUrl: '',
    tgToken: '',
    feishuAppId: '',
    feishuAppSecret: '',
  });

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const pollGenerationRef = useRef(0);
  const operationGenerationRef = useRef(0);
  const pendingOwnerRef = useRef<ConnectionOwner | null>(null);
  const connectionOwnerRef = useRef<ConnectionOwner | null>(null);
  const pendingStartRef = useRef<{
    owner: ConnectionOwner;
    generation: number;
    promise: Promise<ConnectionResult>;
  } | null>(null);
  const cleanupPromiseRef = useRef<Promise<void>>(Promise.resolve());
  const isOpenRef = useRef(isOpen);
  connectionOwnerRef.current = connectionOwner;
  isOpenRef.current = isOpen;

  // ── Derived state ────────────────────────────────────────────────

  const isRelayConnected = status?.pairing_state === 'connected';
  const isBotConnected = !!status?.bot_connected;
  const connectedNetworkTab = methodToNetworkTab(status?.active_method);
  const connectedBotTab = botInfoToBotTab(status?.bot_connected);

  const cancelPendingWork = useCallback(async () => {
    operationGenerationRef.current += 1;
    pollGenerationRef.current += 1;
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = null;

    const owner = pendingOwnerRef.current ?? connectionOwnerRef.current;
    const pendingStart = pendingStartRef.current;
    pendingOwnerRef.current = null;
    connectionOwnerRef.current = null;
    setConnectionOwner(null);
    setConnectionResult(null);
    setWeixinQrSessionKey(null);
    setWeixinQrImageUrl(null);
    setWeixinAwaitingPhoneConfirm(false);
    setLoading(false);

    const previousCleanup = cleanupPromiseRef.current;
    const cleanup = previousCleanup
      .catch(() => undefined)
      .then(async () => {
        // If start is still in flight, stopping before it settles can leak a
        // connection that is created after the stop call. Wait, then stop.
        await stopAfterPendingStart(pendingStart?.promise ?? null, async () => {
          if (pendingStart && pendingStartRef.current === pendingStart) {
            pendingStartRef.current = null;
          }
          try {
            if (owner === 'bot') {
              await remoteConnectAPI.stopBot();
            } else if (owner === 'network') {
              await remoteConnectAPI.stopConnection();
            }
          } catch {
            // Best-effort cleanup; the generation still blocks late UI writes.
          }
        });
      });
    cleanupPromiseRef.current = cleanup;
    await cleanup;
  }, []);

  const handleDialogClose = useCallback(() => {
    void cancelPendingWork();
    onClose();
  }, [cancelPendingWork, onClose]);

  const handleGroupChange = useCallback((nextGroup: ActiveGroup) => {
    if (nextGroup === activeGroup) return;
    void cancelPendingWork();
    setActiveGroup(nextGroup);
    setError(null);
  }, [activeGroup, cancelPendingWork]);

  useEffect(() => {
    if (!isOpen) void cancelPendingWork();
  }, [cancelPendingWork, isOpen]);

  // ── Polling ──────────────────────────────────────────────────────

  const startPolling = useCallback((target: 'relay' | 'bot') => {
    const pollGeneration = ++pollGenerationRef.current;
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      try {
        const s = await remoteConnectAPI.getStatus();
        if (!isOpenRef.current || pollGenerationRef.current !== pollGeneration) return;
        setStatus(s);
        const done = target === 'relay'
          ? s.pairing_state === 'connected'
          : !!s.bot_connected;
        if (done) {
          if (pollRef.current) clearInterval(pollRef.current);
          pollRef.current = null;
          pendingOwnerRef.current = null;
          connectionOwnerRef.current = null;
          setConnectionOwner(null);
          setConnectionResult(null);
        }
      } catch { /* ignore */ }
    }, 2000);
  }, []);

  // On dialog open: check if a connection (restored bot / ongoing relay) is active.
  useEffect(() => {
    if (!isOpen) {
      if (pollRef.current) clearInterval(pollRef.current);
      pollRef.current = null;
      return;
    }

    const agreed = getRemoteConnectDisclaimerAgreed();
    setHasAgreedDisclaimer(agreed);
    if (!agreed) return;

    let cancelled = false;
    const checkExisting = async () => {
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          const s = await remoteConnectAPI.getStatus();
          if (cancelled) return;
          setStatus(s);
          setBotVerboseMode(s.bot_verbose_mode);

          if (s.bot_connected) {
            const tab = botInfoToBotTab(s.bot_connected);
            setActiveGroup('bot');
            if (tab) setBotTab(tab);
            return;
          }
          if (s.pairing_state === 'connected') {
            const tab = methodToNetworkTab(s.active_method);
            setActiveGroup('network');
            if (tab) setNetworkTab(tab);
            return;
          }
          if (['waiting_for_scan', 'verifying', 'handshaking'].includes(s.pairing_state)) {
            const tab = methodToNetworkTab(s.active_method);
            setActiveGroup('network');
            if (tab) setNetworkTab(tab);
            pendingOwnerRef.current = 'network';
            connectionOwnerRef.current = 'network';
            setConnectionOwner('network');
            // Status cannot recover the original QR payload. Restore an
            // explicit in-progress surface with a cancel action instead of
            // silently showing the configuration form or restarting pairing.
            setConnectionResult({
              method: s.active_method ?? tab ?? 'relay',
              qr_data: null,
              qr_svg: null,
              qr_url: null,
              bot_pairing_code: null,
              bot_link: null,
              pairing_state: s.pairing_state,
            });
            startPolling('relay');
            return;
          }
        } catch { /* ignore */ }
        if (attempt < 2) {
          await new Promise(r => setTimeout(r, 1500));
          if (cancelled) return;
        }
      }
    };
    void checkExisting();
    return () => {
      cancelled = true;
      if (pollRef.current) clearInterval(pollRef.current);
      pollGenerationRef.current += 1;
    };
  }, [isOpen, hasAgreedDisclaimer, startPolling]);

  useEffect(() => {
    if (!isOpen || !hasAgreedDisclaimer || activeGroup !== 'network' || networkTab !== 'lan') return;
    let cancelled = false;
    const loadLanNetworkInfo = async () => {
      const info = await remoteConnectAPI.getLanNetworkInfo();
      if (!cancelled && info) {
        const availableIps = info.available_ips ?? [];
        setLanNetworkInfo({
          localIp: info.local_ip,
          gatewayIp: info.gateway_ip ?? null,
          availableIps,
        });
        // Auto-select the first (highest-priority) IP if nothing is selected yet
        // or the previous selection is no longer in the list.
        setSelectedLanIp(prev => {
          if (prev && availableIps.some(e => e.ip === prev)) return prev;
          return availableIps[0]?.ip ?? info.local_ip ?? '';
        });
      }
    };
    void loadLanNetworkInfo();
    return () => {
      cancelled = true;
    };
  }, [isOpen, hasAgreedDisclaimer, activeGroup, networkTab]);

  useEffect(() => {
    if (!isOpen || !hasAgreedDisclaimer) return;
    let cancelled = false;
    const loadFormState = async () => {
      try {
        const formState = await remoteConnectAPI.getFormState();
        if (cancelled) return;
        setCustomUrl(formState.custom_server_url ?? '');
        setTgToken(formState.telegram_bot_token ?? '');
        setFeishuAppId(formState.feishu_app_id ?? '');
        setFeishuAppSecret(formState.feishu_app_secret ?? '');
        setWeixinIlinkToken(formState.weixin_ilink_token ?? '');
        setWeixinBaseUrl(formState.weixin_base_url ?? '');
        setWeixinBotAccountId(formState.weixin_bot_account_id ?? '');
      } catch {
        // Ignore form-state restore failures and keep in-memory defaults.
      }
    };
    void loadFormState();
    return () => {
      cancelled = true;
    };
  }, [isOpen, hasAgreedDisclaimer]);

  // Keep the Self-Hosted server URL in sync with account login state. The
  // backend already persists the mirrored value; this refreshes the input
  // while the dialog is open (fill on login, clear on logout).
  useEffect(() => {
    const unlisten = api.listen<{ logged_in: boolean; relay_url?: string }>(
      'account://login-state',
      (payload) => {
        if (payload?.logged_in && payload.relay_url) {
          setCustomUrl(payload.relay_url);
        } else if (payload && !payload.logged_in) {
          setCustomUrl('');
        }
      },
    );
    return () => {
      unlisten();
    };
  }, []);

  // The account status and pairing status intentionally expose opaque UUIDs
  // for identity checks. Resolve the persisted, non-secret login hint for the
  // user-facing connected state instead.
  useEffect(() => {
    if (!isOpen || !accountLoggedIn) {
      setAccountUsername(null);
      return;
    }
    let cancelled = false;
    void remoteConnectAPI.accountGetCredentialHint().then((hint) => {
      if (!cancelled) {
        setAccountUsername(hint?.username.trim() || null);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [accountLoggedIn, isOpen]);

  useEffect(() => {
    formSnapshotRef.current = {
      customUrl,
      tgToken,
      feishuAppId,
      feishuAppSecret,
    };
  }, [customUrl, tgToken, feishuAppId, feishuAppSecret]);

  const prepareAndStartWeixinBotFromQr = useCallback(async (
    ilinkToken: string,
    baseUrl: string,
    botAccountId: string,
  ): Promise<ConnectionResult> => {
    const fs = formSnapshotRef.current;
    await remoteConnectAPI.setFormState({
      custom_server_url: fs.customUrl,
      telegram_bot_token: fs.tgToken,
      feishu_app_id: fs.feishuAppId,
      feishu_app_secret: fs.feishuAppSecret,
      weixin_ilink_token: ilinkToken,
      weixin_base_url: baseUrl || undefined,
      weixin_bot_account_id: botAccountId,
    });
    await remoteConnectAPI.configureBot({
      botType: 'weixin',
      weixinIlinkToken: ilinkToken,
      weixinBaseUrl: baseUrl || undefined,
      weixinBotAccountId: botAccountId,
    });
    return await remoteConnectAPI.startConnection('bot_weixin');
  }, []);

  // WeChat QR login: poll iLink until confirmed or error (session key cleared on completion).
  useEffect(() => {
    const key = weixinQrSessionKey;
    if (!key) return;
    const operationGeneration = operationGenerationRef.current;
    let cancelled = false;
    const isCurrent = () => (
      !cancelled
      && isOpenRef.current
      && operationGenerationRef.current === operationGeneration
      && pendingOwnerRef.current === 'bot'
    );
    void (async () => {
      while (isCurrent()) {
        try {
          const p = await remoteConnectAPI.weixinQrPoll(key);
          if (!isCurrent()) return;
          if (p.status === 'scanned') {
            setWeixinQrImageUrl(null);
            setWeixinAwaitingPhoneConfirm(true);
            await new Promise(resolve => setTimeout(resolve, 750));
            continue;
          }
          if (p.status === 'confirmed' && p.ilink_token && p.bot_account_id) {
            const token = p.ilink_token;
            const base = p.base_url ?? '';
            const bid = p.bot_account_id;
            setWeixinAwaitingPhoneConfirm(false);
            setWeixinIlinkToken(token);
            setWeixinBaseUrl(base);
            setWeixinBotAccountId(bid);
            // Hide QR immediately, but keep `weixinQrSessionKey` until the pipeline finishes.
            // Clearing the session key first re-runs this effect's cleanup and sets `cancelled`,
            // so after `await` we would skip `setConnectionResult` and never `setLoading(false)`.
            setWeixinQrImageUrl(null);
            setConnectionResult(null);
            setError(null);
            setLoading(true);
            try {
              await cleanupPromiseRef.current.catch(() => undefined);
              if (!isCurrent()) return;
              const startPromise = prepareAndStartWeixinBotFromQr(token, base, bid);
              const pendingStart = {
                owner: 'bot' as const,
                generation: operationGeneration,
                promise: startPromise,
              };
              pendingStartRef.current = pendingStart;
              const result = await startPromise;
              if (pendingStartRef.current === pendingStart) pendingStartRef.current = null;
              if (isCurrent()) {
                connectionOwnerRef.current = 'bot';
                setConnectionOwner('bot');
                setConnectionResult(result);
                startPolling('bot');
              }
            } catch (e: unknown) {
              if (pendingStartRef.current?.generation === operationGeneration) {
                pendingStartRef.current = null;
              }
              if (isCurrent()) {
                setError(e instanceof Error ? e.message : String(e));
              }
            } finally {
              if (isCurrent()) {
                setLoading(false);
              }
            }
            if (isCurrent()) {
              setWeixinQrSessionKey(null);
            }
            return;
          }
          if (p.status === 'error') {
            pendingOwnerRef.current = null;
            setError(p.message);
            setWeixinQrSessionKey(null);
            setWeixinQrImageUrl(null);
            setWeixinAwaitingPhoneConfirm(false);
            return;
          }
          if (p.status === 'expired' && p.qr_image_url) {
            setWeixinQrImageUrl(p.qr_image_url);
            setWeixinAwaitingPhoneConfirm(false);
          }
          await new Promise(resolve => setTimeout(resolve, 750));
        } catch (e: unknown) {
          updateIfOperationCurrent(isCurrent, () => {
            pendingOwnerRef.current = null;
            setError(e instanceof Error ? e.message : String(e));
            setWeixinQrSessionKey(null);
            setWeixinQrImageUrl(null);
            setWeixinAwaitingPhoneConfirm(false);
          });
          return;
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [weixinQrSessionKey, prepareAndStartWeixinBotFromQr, startPolling]);

  // ── Connection handlers ──────────────────────────────────────────

  const handleConnect = useCallback(async () => {
    if (!hasAgreedDisclaimer) {
      setShowDisclaimer(true);
      return;
    }
    const owner: ConnectionOwner = activeGroup === 'bot' ? 'bot' : 'network';
    const operationGeneration = ++operationGenerationRef.current;
    pendingOwnerRef.current = owner;
    setLoading(true);
    setError(null);
    setConnectionResult(null);
    setConnectionOwner(null);

    const isCurrent = () => (
      isOpenRef.current
      && operationGenerationRef.current === operationGeneration
      && pendingOwnerRef.current === owner
    );
    let ownsConnection = false;

    try {
      await cleanupPromiseRef.current.catch(() => undefined);
      if (!isCurrent()) return;
      if (activeGroup === 'network' && networkTab === 'custom_server') {
        const relayUrl = parseRelayServer(customUrl);
        if (!relayUrl) {
          setError(t('accountLogin.invalidServer'));
          return;
        }
        const isLoopback = ['localhost', '127.0.0.1', '[::1]', '::1'].includes(relayUrl.hostname);
        if (relayUrl.protocol === 'http:' && !isLoopback) {
          const confirmed = await confirmWarning(
            t('accountLogin.insecureServerTitle'),
            t('accountLogin.insecureServerConfirm'),
            {
              confirmText: t('accountLogin.continueInsecure'),
              cancelText: t('accountLogin.cancel'),
            },
          );
          if (!confirmed || !isCurrent()) return;
        }
      }
      await remoteConnectAPI.setFormState({
        custom_server_url: customUrl,
        telegram_bot_token: tgToken,
        feishu_app_id: feishuAppId,
        feishu_app_secret: feishuAppSecret,
        weixin_ilink_token: weixinIlinkToken,
        weixin_base_url: weixinBaseUrl,
        weixin_bot_account_id: weixinBotAccountId,
      });
      if (!isCurrent()) return;

      let method: string;
      let serverUrl: string | undefined;

      if (activeGroup === 'bot') {
        if (botTab === 'telegram') {
          method = 'bot_telegram';
        } else if (botTab === 'feishu') {
          method = 'bot_feishu';
        } else {
          method = 'bot_weixin';
        }
        if (botTab === 'telegram' && tgToken) {
          await remoteConnectAPI.configureBot({ botType: 'telegram', botToken: tgToken });
        } else if (botTab === 'feishu' && feishuAppId) {
          await remoteConnectAPI.configureBot({
            botType: 'feishu', appId: feishuAppId, appSecret: feishuAppSecret,
          });
        } else if (botTab === 'weixin' && weixinIlinkToken && weixinBotAccountId) {
          await remoteConnectAPI.configureBot({
            botType: 'weixin',
            weixinIlinkToken: weixinIlinkToken,
            weixinBaseUrl: weixinBaseUrl || undefined,
            weixinBotAccountId: weixinBotAccountId,
          });
        }
        if (!isCurrent()) return;
      } else {
        method = networkTab;
        if (networkTab === 'custom_server') serverUrl = customUrl || undefined;
      }
      const lanIp = networkTab === 'lan' ? (selectedLanIp || undefined) : undefined;
      const startPromise = remoteConnectAPI.startConnection(method, serverUrl, lanIp);
      const pendingStart = { owner, generation: operationGeneration, promise: startPromise };
      pendingStartRef.current = pendingStart;
      const result = await startPromise;
      if (pendingStartRef.current === pendingStart) pendingStartRef.current = null;
      if (!isCurrent()) return;
      connectionOwnerRef.current = owner;
      setConnectionOwner(owner);
      setConnectionResult(result);
      ownsConnection = true;
      startPolling(owner === 'bot' ? 'bot' : 'relay');
    } catch (e: any) {
      if (pendingStartRef.current?.generation === operationGeneration) {
        pendingStartRef.current = null;
      }
      if (isCurrent()) {
        pendingOwnerRef.current = null;
        setError(e?.message || String(e));
      }
    } finally {
      if (isCurrent()) {
        if (!ownsConnection) pendingOwnerRef.current = null;
        setLoading(false);
      }
    }
  }, [activeGroup, networkTab, botTab, customUrl, tgToken, feishuAppId, feishuAppSecret, weixinIlinkToken, weixinBaseUrl, weixinBotAccountId, selectedLanIp, startPolling, t, hasAgreedDisclaimer]);

  const handleStartWeixinQr = useCallback(async () => {
    if (!hasAgreedDisclaimer) {
      setShowDisclaimer(true);
      return;
    }
    const operationGeneration = ++operationGenerationRef.current;
    pendingOwnerRef.current = 'bot';
    setError(null);
    setWeixinAwaitingPhoneConfirm(false);
    setLoading(true);
    try {
      await cleanupPromiseRef.current.catch(() => undefined);
      if (!isOpenRef.current || operationGenerationRef.current !== operationGeneration) return;
      const r = await remoteConnectAPI.weixinQrStart(null);
      if (!isOpenRef.current || operationGenerationRef.current !== operationGeneration) return;
      setWeixinQrSessionKey(r.session_key);
      setWeixinQrImageUrl(r.qr_image_url);
    } catch (e: unknown) {
      if (isOpenRef.current && operationGenerationRef.current === operationGeneration) {
        pendingOwnerRef.current = null;
        setError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      if (isOpenRef.current && operationGenerationRef.current === operationGeneration) {
        setLoading(false);
      }
    }
  }, [hasAgreedDisclaimer]);

  const handleCancelWeixinQr = useCallback(() => {
    void cancelPendingWork();
  }, [cancelPendingWork]);

  const handleDisconnectRelay = useCallback(async () => {
    try {
      await remoteConnectAPI.stopConnection();
      pendingOwnerRef.current = null;
      connectionOwnerRef.current = null;
      setConnectionOwner(null);
      setConnectionResult(null);
      const s = await remoteConnectAPI.getStatus();
      setStatus(s);
    } catch { /* best effort */ }
  }, []);

  const handleDisconnectBot = useCallback(async () => {
    try {
      await remoteConnectAPI.stopBot();
      pendingOwnerRef.current = null;
      connectionOwnerRef.current = null;
      setConnectionOwner(null);
      setConnectionResult(null);
      const s = await remoteConnectAPI.getStatus();
      setStatus(s);
    } catch { /* best effort */ }
  }, []);

  const handleToggleBotVerboseMode = async () => {
    const newMode = !botVerboseMode;
    setBotVerboseMode(newMode);
    await remoteConnectAPI.setBotVerboseMode(newMode);
  };

  const handleCancelConnect = useCallback(async () => {
    await cancelPendingWork();
    if (!isOpenRef.current) return;
    try {
      const s = await remoteConnectAPI.getStatus();
      if (isOpenRef.current) setStatus(s);
    } catch { /* best effort */ }
  }, [cancelPendingWork]);

  const handleOpenNgrokSetup = useCallback(() => {
    void systemAPI.openExternal(NGROK_SETUP_URL);
  }, []);

  /** Self-Hosted tab entry: open the in-app wizard, never an external README. */
  const handleOpenRelayDeploy = useCallback(() => {
    setShowRelayDeploy(true);
  }, []);

  const handleRelayDeployRegistered = useCallback((result: RelayDeployResult) => {
    setShowRelayDeploy(false);
    setCustomUrl(result.relayUrl);
    setNetworkTab('custom_server');
    setActiveGroup('network');
    setError(null);
  }, []);

  const handleOpenFeishuGuide = useCallback(() => {
    void systemAPI.openExternal(pickLocalizedUrl(FEISHU_SETUP_GUIDE_URLS, currentLanguage));
  }, [currentLanguage]);

  const renderInfoCard = (children: React.ReactNode) => (
    <div className="bitfun-remote-connect__info-card">
      {children}
    </div>
  );

  // ── Sub-tab disabled logic ───────────────────────────────────────

  const isNetworkSubDisabled = (tabId: NetworkTab): boolean => {
    if (isRelayConnected && connectedNetworkTab && connectedNetworkTab !== tabId) return true;
    return false;
  };

  const isBotSubDisabled = (tabId: BotTab): boolean => {
    if (isBotConnected && connectedBotTab && connectedBotTab !== tabId) return true;
    return false;
  };

  // ── Renderers ────────────────────────────────────────────────────

  const renderErrorBlock = () => {
    if (!error) return null;
    const isNgrokErr = error.includes('ngrok is not installed');
    return (
      <div className="bitfun-remote-connect__error-group">
        <p className="bitfun-remote-connect__error">{error}</p>
        {isNgrokErr && (
          <button type="button" className="bitfun-remote-connect__error-action" onClick={handleOpenNgrokSetup}>
            {t('remoteConnect.openNgrokSetup')}
          </button>
        )}
      </div>
    );
  };

  const renderConnectedView = (
    onDisconnect: () => void,
    username?: string | null,
  ) => (
    <div className="bitfun-remote-connect__connected">
      <div className="bitfun-remote-connect__status">
        <Badge variant="success">{t('remoteConnect.stateConnected')}</Badge>
        {username && (
          <span className="bitfun-remote-connect__peer-username">
            {t('accountLogin.username')}: {username}
          </span>
        )}
      </div>
      <p className="bitfun-remote-connect__hint">{t('remoteConnect.connectedHint')}</p>
      <button type="button" className="bitfun-remote-connect__btn bitfun-remote-connect__btn--disconnect" onClick={onDisconnect}>
        {t('remoteConnect.disconnect')}
      </button>
    </div>
  );

  const handleCopyPairingUrl = useCallback(async () => {
    if (!connectionResult?.qr_url) return;
    const copied = await copyTextToClipboard(connectionResult.qr_url);
    if (copied) {
      setQrCopied(true);
      window.setTimeout(() => setQrCopied(false), 2000);
    } else {
      notifyError(t('remoteConnect.copyUrlFailed'));
    }
  }, [connectionResult?.qr_url, notifyError, t]);

  const renderPairingInProgress = () => {
    if (!connectionResult) return null;
    return (
      <div className="bitfun-remote-connect__body">
        {connectionResult.qr_url && (
          <button
            type="button"
            className="bitfun-remote-connect__qr-box"
            title={t('remoteConnect.copyUrl')}
            aria-label={t('remoteConnect.copyUrl')}
            onClick={() => void handleCopyPairingUrl()}
          >
            <QRCodeSVG value={connectionResult.qr_url} size={180} level="M" includeMargin />
          </button>
        )}
        {connectionResult.bot_pairing_code && (
          <div className="bitfun-remote-connect__pairing-code-box">
            <div className="bitfun-remote-connect__pairing-code">
              {connectionResult.bot_pairing_code}
            </div>
          </div>
        )}
        <div className="bitfun-remote-connect__status">
          <Badge variant={qrCopied ? 'success' : 'warning'}>
            {qrCopied
              ? t('remoteConnect.urlCopied')
              : connectionOwner === 'bot'
                ? t('remoteConnect.stateWaitingBot')
                : t('remoteConnect.stateWaiting')}
          </Badge>
        </div>
        <p className="bitfun-remote-connect__hint">
          {connectionOwner === 'bot'
            ? t('remoteConnect.botHint')
            : connectionResult.qr_url
              ? t('remoteConnect.scanHint')
              : t('remoteConnect.stateWaiting')}
        </p>
        <button type="button" className="bitfun-remote-connect__btn bitfun-remote-connect__btn--cancel" onClick={handleCancelConnect}>
          {t('remoteConnect.cancel')}
        </button>
      </div>
    );
  };

  // ── Network group content ────────────────────────────────────────

  const NGROK_USAGE_URL = 'https://dashboard.ngrok.com/legacy/usage';

  const renderNetworkContent = () => {
    if (isRelayConnected && connectedNetworkTab === networkTab) {
      return (
        <>
          {networkTab === 'ngrok' && (
            <p className="bitfun-remote-connect__ngrok-usage-link">
              <span
                className="bitfun-remote-connect__description-link"
                role="link"
                tabIndex={0}
                onClick={() => systemAPI.openExternal(NGROK_USAGE_URL)}
                onKeyDown={(e) => { if (e.key === 'Enter') systemAPI.openExternal(NGROK_USAGE_URL); }}
              >
                {t('remoteConnect.ngrokUsageLink')}
              </span>
            </p>
          )}
          {renderConnectedView(
            handleDisconnectRelay,
            accountUsername,
          )}
        </>
      );
    }
    if (connectionResult && connectionOwner === 'network') {
      return renderPairingInProgress();
    }
    return (
      <div className="bitfun-remote-connect__body">
        {renderInfoCard(
          <>
            {networkTab === 'lan' && (lanNetworkInfo?.availableIps.length || lanNetworkInfo?.gatewayIp) && (
              <div className="bitfun-remote-connect__info-meta-group">
                {lanNetworkInfo && lanNetworkInfo.availableIps.length > 0 && (
                  <div className="bitfun-remote-connect__lan-ip-select">
                    <span className="bitfun-remote-connect__info-meta-label">
                      {t('remoteConnect.currentIp')}
                    </span>
                    <Select
                      className="bitfun-remote-connect__lan-ip-dropdown"
                      size="small"
                      value={selectedLanIp}
                      onChange={(v) => setSelectedLanIp(String(v))}
                      options={lanNetworkInfo.availableIps.map(e => ({
                        label: e.ip,
                        value: e.ip,
                        description: e.interface_name,
                      }))}
                    />
                  </div>
                )}
                {(() => {
                  const selectedIntf = lanNetworkInfo?.availableIps.find(e => e.ip === selectedLanIp);
                  const gw = selectedIntf?.gateway_ip ?? null;
                  if (!gw) return null;
                  return (
                    <p className="bitfun-remote-connect__info-meta">
                      {t('remoteConnect.gatewayIp')}: {gw}
                    </p>
                  );
                })()}
              </div>
            )}
            <p className="bitfun-remote-connect__info-text">
              {networkTab === 'custom_server' ? (
                <>
                  {t('remoteConnect.desc_custom_server_prefix')}
                  <span
                    className="bitfun-remote-connect__description-link"
                    role="link"
                    tabIndex={0}
                    onClick={handleOpenRelayDeploy}
                    onKeyDown={(e) => { if (e.key === 'Enter') handleOpenRelayDeploy(); }}
                  >
                    {t('remoteConnect.desc_custom_server_link')}
                  </span>
                  {t('remoteConnect.desc_custom_server_suffix')}
                </>
              ) : networkTab === 'ngrok' ? (
                <>
                  {t('remoteConnect.desc_ngrok_prefix')}
                  <span
                    className="bitfun-remote-connect__description-link"
                    role="link"
                    tabIndex={0}
                    onClick={handleOpenNgrokSetup}
                    onKeyDown={(e) => { if (e.key === 'Enter') handleOpenNgrokSetup(); }}
                  >
                    {t('remoteConnect.desc_ngrok_link')}
                  </span>
                  {t('remoteConnect.desc_ngrok_suffix')}
                </>
              ) : (
                t(`remoteConnect.desc_${networkTab}`)
              )}
            </p>
          </>,
        )}
        {networkTab === 'custom_server' && (
          <Input
            className="bitfun-remote-connect__field bitfun-remote-connect__field--inline"
            type="url"
            placeholder="https://relay.example.com:9700"
            prefix={<span className="bitfun-remote-connect__field-prefix">{t('remoteConnect.serverUrl')}</span>}
            value={customUrl}
            onChange={(e) => setCustomUrl(e.target.value)}
          />
        )}
        {renderErrorBlock()}
        <button
          type="button"
          className="bitfun-remote-connect__btn bitfun-remote-connect__btn--connect"
          onClick={handleConnect} disabled={loading}
        >
          {loading ? t('remoteConnect.connecting') : t('remoteConnect.connect')}
        </button>
      </div>
    );
  };

  // ── Bot group content ────────────────────────────────────────────

  const renderBotContent = () => {
    if (isBotConnected && connectedBotTab === botTab) {
      return (
        <div className="bitfun-remote-connect__connected">
          <div className="bitfun-remote-connect__status">
            <Badge variant="success">{t('remoteConnect.stateConnected')}</Badge>
          </div>
          <div className="bitfun-remote-connect__mode-selector">
            <button
              type="button"
              className={`bitfun-remote-connect__mode-btn ${!botVerboseMode ? 'is-active' : ''}`}
              onClick={botVerboseMode ? handleToggleBotVerboseMode : undefined}
            >
              {t('remoteConnect.botConciseMode')}
            </button>
            <button
              type="button"
              className={`bitfun-remote-connect__mode-btn ${botVerboseMode ? 'is-active' : ''}`}
              onClick={!botVerboseMode ? handleToggleBotVerboseMode : undefined}
            >
              {t('remoteConnect.botVerboseMode')}
            </button>
          </div>
          <button
            type="button"
            className="bitfun-remote-connect__btn bitfun-remote-connect__btn--disconnect"
            onClick={handleDisconnectBot}
          >
            {t('remoteConnect.disconnect')}
          </button>
        </div>
      );
    }
    if (connectionResult && connectionOwner === 'bot') {
      return renderPairingInProgress();
    }
    return (
      <div className="bitfun-remote-connect__body">
        {botTab === 'telegram' ? (
          <div className="bitfun-remote-connect__bot-guide">
            {renderInfoCard(
              <div className="bitfun-remote-connect__steps">
                <p className="bitfun-remote-connect__step">1. {t('remoteConnect.botTgStep1')}</p>
                <p className="bitfun-remote-connect__step">2. {t('remoteConnect.botTgStep2')}</p>
                <p className="bitfun-remote-connect__step">3. {t('remoteConnect.botTgStep3')}</p>
              </div>,
            )}
            <Input
              className="bitfun-remote-connect__field bitfun-remote-connect__field--inline"
              type="text"
              placeholder="123456:xxxxxxxxxxxxxxxxxxxxxxxx"
              prefix={<span className="bitfun-remote-connect__field-prefix">Bot Token</span>}
              value={tgToken}
              onChange={(e) => setTgToken(e.target.value)}
            />
          </div>
        ) : botTab === 'feishu' ? (
          <div className="bitfun-remote-connect__bot-guide">
            {renderInfoCard(
              <>
                <p className="bitfun-remote-connect__info-text">
                  {t('remoteConnect.botFeishuDocPrefix')}
                  <span
                    className="bitfun-remote-connect__description-link"
                    role="link"
                    tabIndex={0}
                    onClick={handleOpenFeishuGuide}
                    onKeyDown={(e) => { if (e.key === 'Enter') handleOpenFeishuGuide(); }}
                  >
                    {t('remoteConnect.botFeishuDocLink')}
                  </span>
                  {t('remoteConnect.botFeishuDocSuffix')}
                </p>
                <div className="bitfun-remote-connect__steps">
                  <p className="bitfun-remote-connect__step">
                    1. {t('remoteConnect.botFeishuStep1Prefix')}
                    <span
                      className="bitfun-remote-connect__step-link"
                      role="link"
                      tabIndex={0}
                      onClick={() => systemAPI.openExternal('https://open.feishu.cn/app')}
                      onKeyDown={(e) => { if (e.key === 'Enter') systemAPI.openExternal('https://open.feishu.cn/app'); }}
                    >
                      {t('remoteConnect.botFeishuOpenPlatform')}
                    </span>
                    {t('remoteConnect.botFeishuStep1Suffix')}
                  </p>
                  <p className="bitfun-remote-connect__step">2. {t('remoteConnect.botFeishuStep2')}</p>
                  <p className="bitfun-remote-connect__step">3. {t('remoteConnect.botFeishuStep3')}</p>
                </div>
              </>,
            )}
            <Input
              className="bitfun-remote-connect__field bitfun-remote-connect__field--inline"
              type="text"
              placeholder="cli_xxxxxxxx"
              prefix={<span className="bitfun-remote-connect__field-prefix">App ID</span>}
              value={feishuAppId}
              onChange={(e) => setFeishuAppId(e.target.value)}
            />
            <Input
              className="bitfun-remote-connect__field bitfun-remote-connect__field--inline"
              type="password"
              placeholder="xxxxxxxxxxxxxxxx"
              prefix={<span className="bitfun-remote-connect__field-prefix">App Secret</span>}
              value={feishuAppSecret}
              onChange={(e) => setFeishuAppSecret(e.target.value)}
            />
          </div>
        ) : (
          <div className="bitfun-remote-connect__bot-guide">
            {renderInfoCard(
              <div className="bitfun-remote-connect__steps">
                <p className="bitfun-remote-connect__info-text">{t('remoteConnect.botWeixinIntro')}</p>
                <p className="bitfun-remote-connect__step">1. {t('remoteConnect.botWeixinStep1')}</p>
                <p className="bitfun-remote-connect__step">2. {t('remoteConnect.botWeixinStep2')}</p>
              </div>,
            )}
            {weixinQrImageUrl && (
              <div className="bitfun-remote-connect__weixin-qr">
                {isWeixinRasterQrSrc(weixinQrImageUrl) ? (
                  <img
                    src={weixinQrImageUrl}
                    alt={t('remoteConnect.weixinQrAlt')}
                    className="bitfun-remote-connect__weixin-qr-img"
                  />
                ) : (
                  <div
                    className="bitfun-remote-connect__weixin-qr-svg-wrap"
                    role="img"
                    aria-label={t('remoteConnect.weixinQrAlt')}
                  >
                    <QRCodeSVG
                      value={weixinQrImageUrl}
                      size={200}
                      level="M"
                      includeMargin
                    />
                  </div>
                )}
                <p className="bitfun-remote-connect__hint">{t('remoteConnect.botWeixinPolling')}</p>
                <button
                  type="button"
                  className="bitfun-remote-connect__btn bitfun-remote-connect__btn--cancel"
                  onClick={handleCancelWeixinQr}
                >
                  {t('remoteConnect.botWeixinQrCancel')}
                </button>
              </div>
            )}
            {weixinQrSessionKey && !weixinQrImageUrl && weixinAwaitingPhoneConfirm && (
              <div className="bitfun-remote-connect__weixin-qr bitfun-remote-connect__weixin-qr--await">
                <p className="bitfun-remote-connect__hint">{t('remoteConnect.botWeixinAwaitingPhoneConfirm')}</p>
                <button
                  type="button"
                  className="bitfun-remote-connect__btn bitfun-remote-connect__btn--cancel"
                  onClick={handleCancelWeixinQr}
                >
                  {t('remoteConnect.botWeixinQrCancel')}
                </button>
              </div>
            )}
            {!weixinQrSessionKey && !weixinQrImageUrl && (
              <button
                type="button"
                className="bitfun-remote-connect__btn bitfun-remote-connect__btn--cancel"
                onClick={handleStartWeixinQr}
                disabled={loading}
              >
                {t('remoteConnect.botWeixinQrButton')}
              </button>
            )}
            {weixinIlinkToken && weixinBotAccountId && !weixinQrSessionKey && (
              <p className="bitfun-remote-connect__hint">{t('remoteConnect.botWeixinLinked')}</p>
            )}
          </div>
        )}
        {renderErrorBlock()}
        <button
          type="button"
          className="bitfun-remote-connect__btn bitfun-remote-connect__btn--connect"
          onClick={handleConnect}
          disabled={
            loading
            || (botTab === 'telegram' ? !tgToken
              : botTab === 'feishu' ? !feishuAppId
                : !weixinIlinkToken || !weixinBotAccountId)
          }
        >
          {loading ? t('remoteConnect.connecting') : t('remoteConnect.connect')}
        </button>
      </div>
    );
  };

  // ── Layout ───────────────────────────────────────────────────────

  const isNetworkConnecting = !!connectionResult && connectionOwner === 'network' && !isRelayConnected;
  const isBotConnecting = !!connectionResult && connectionOwner === 'bot' && !isBotConnected;
  const handleAgreeDisclaimer = useCallback(() => {
    setRemoteConnectDisclaimerAgreed();
    setHasAgreedDisclaimer(true);
    setShowDisclaimer(false);
  }, []);

  useEffect(() => {
    if (isOpen && hasAgreedDisclaimer && !hasWorkspace && activeGroup !== 'account') {
      handleGroupChange('account');
    }
  }, [activeGroup, handleGroupChange, hasAgreedDisclaimer, hasWorkspace, isOpen]);

  const disclaimerIsGate = isOpen && !hasAgreedDisclaimer;
  const handleDisclaimerClose = disclaimerIsGate
    ? handleDialogClose
    : () => setShowDisclaimer(false);

  return (
    <>
      <Modal
        isOpen={isOpen && hasAgreedDisclaimer}
        onClose={handleDialogClose}
        title={t('shared:features.remoteControl')}
        titleExtra={(
          <span className="bitfun-remote-connect__title-extra">
            <button
              type="button"
              className="bitfun-remote-connect__disclaimer-trigger"
              onClick={() => setShowDisclaimer(true)}
            >
              {t('remoteConnect.disclaimerReview')}
            </button>
          </span>
        )}
        showCloseButton
        size="large"
      >
        <div className="bitfun-remote-connect">
          {/* ── Group tabs ── */}
          <div className="bitfun-remote-connect__groups" role="tablist" aria-label={t('shared:features.remoteControl')}>
            <Tooltip
              content={t('header.remoteConnectRequiresWorkspace')}
              placement="bottom"
              disabled={hasWorkspace}
            >
              <button
                type="button"
                role="tab"
                id="remote-connect-group-network"
                aria-controls="remote-connect-panel-network"
                aria-selected={activeGroup === 'network'}
                tabIndex={activeGroup === 'network' ? 0 : -1}
                onKeyDown={handleTabArrowKey}
                className={`bitfun-remote-connect__group-btn${activeGroup === 'network' ? ' is-active' : ''}`}
                onClick={() => handleGroupChange('network')}
                disabled={!hasWorkspace}
              >
                {t('remoteConnect.groupNetwork')}
                {isRelayConnected && <span className="bitfun-remote-connect__dot" />}
              </button>
            </Tooltip>
            <span className="bitfun-remote-connect__group-divider" />
            <Tooltip
              content={t('header.remoteConnectRequiresWorkspace')}
              placement="bottom"
              disabled={hasWorkspace}
            >
              <button
                type="button"
                role="tab"
                id="remote-connect-group-bot"
                aria-controls="remote-connect-panel-bot"
                aria-selected={activeGroup === 'bot'}
                tabIndex={activeGroup === 'bot' ? 0 : -1}
                onKeyDown={handleTabArrowKey}
                className={`bitfun-remote-connect__group-btn${activeGroup === 'bot' ? ' is-active' : ''}`}
                onClick={() => handleGroupChange('bot')}
                disabled={!hasWorkspace}
              >
                {t('remoteConnect.groupBot')}
                {isBotConnected && <span className="bitfun-remote-connect__dot" />}
              </button>
            </Tooltip>
            <span className="bitfun-remote-connect__group-divider" />
            <button
              type="button"
              role="tab"
              id="remote-connect-group-account"
              aria-controls="remote-connect-panel-account"
              aria-selected={activeGroup === 'account'}
              tabIndex={activeGroup === 'account' || !hasWorkspace ? 0 : -1}
              onKeyDown={handleTabArrowKey}
              className={`bitfun-remote-connect__group-btn${activeGroup === 'account' ? ' is-active' : ''}`}
              onClick={() => handleGroupChange('account')}
            >
              {t('remoteConnect.groupAccount')}
              {accountLoggedIn && <span className="bitfun-remote-connect__dot" />}
            </button>
          </div>

          {/* ── Sub-tabs ── */}
          {activeGroup === 'account' ? null : activeGroup === 'network' ? (
            <div className="bitfun-remote-connect__subtabs" role="tablist" aria-label={t('remoteConnect.groupNetwork')}>
              {NETWORK_TABS.map((tab, i) => (
                <React.Fragment key={tab.id}>
                  {i > 0 && <span className="bitfun-remote-connect__subtab-divider" />}
                  <button
                    type="button"
                    role="tab"
                    id={`remote-connect-network-tab-${tab.id}`}
                    aria-controls="remote-connect-network-tabpanel"
                    aria-selected={networkTab === tab.id}
                    tabIndex={networkTab === tab.id ? 0 : -1}
                    onKeyDown={handleTabArrowKey}
                    className={`bitfun-remote-connect__subtab${networkTab === tab.id ? ' is-active' : ''}${isRelayConnected && connectedNetworkTab === tab.id ? ' is-connected' : ''}`}
                    onClick={() => {
                      if (networkTab !== tab.id) void cancelPendingWork();
                      setNetworkTab(tab.id);
                      setError(null);
                    }}
                    disabled={isNetworkSubDisabled(tab.id) || isNetworkConnecting}
                  >
                    {t(tab.labelKey)}
                    {isRelayConnected && connectedNetworkTab === tab.id && networkTab !== tab.id && (
                      <span className="bitfun-remote-connect__dot-sm" />
                    )}
                  </button>
                </React.Fragment>
              ))}
            </div>
          ) : (
            <div className="bitfun-remote-connect__subtabs" role="tablist" aria-label={t('remoteConnect.groupBot')}>
              {BOT_TABS.map((tab, i) => (
                <React.Fragment key={tab.id}>
                  {i > 0 && <span className="bitfun-remote-connect__subtab-divider" />}
                  <button
                    type="button"
                    role="tab"
                    id={`remote-connect-bot-tab-${tab.id}`}
                    aria-controls="remote-connect-bot-tabpanel"
                    aria-selected={botTab === tab.id}
                    tabIndex={botTab === tab.id ? 0 : -1}
                    onKeyDown={handleTabArrowKey}
                    className={`bitfun-remote-connect__subtab${botTab === tab.id ? ' is-active' : ''}${isBotConnected && connectedBotTab === tab.id ? ' is-connected' : ''}`}
                    onClick={() => {
                      if (botTab !== tab.id) void cancelPendingWork();
                      setBotTab(tab.id);
                      setError(null);
                    }}
                    disabled={isBotSubDisabled(tab.id) || isBotConnecting}
                  >
                    {tab.id === 'feishu' ? t('remoteConnect.feishu') : tab.id === 'weixin' ? t('remoteConnect.weixin') : tab.label}
                    {isBotConnected && connectedBotTab === tab.id && botTab !== tab.id && (
                      <span className="bitfun-remote-connect__dot-sm" />
                    )}
                  </button>
                </React.Fragment>
              ))}
            </div>
          )}

          {/* ── Content ── */}
          {activeGroup === 'account' ? (
            <div
              id="remote-connect-panel-account"
              role="tabpanel"
              aria-labelledby="remote-connect-group-account"
            >
              <AccountPanel onCloseDialog={handleDialogClose} />
            </div>
          ) : activeGroup === 'network' ? (
            <div
              id="remote-connect-panel-network"
              role="tabpanel"
              aria-labelledby="remote-connect-group-network"
            >
              <div
                id="remote-connect-network-tabpanel"
                role="tabpanel"
                aria-labelledby={`remote-connect-network-tab-${networkTab}`}
              >
                {renderNetworkContent()}
              </div>
            </div>
          ) : (
            <div
              id="remote-connect-panel-bot"
              role="tabpanel"
              aria-labelledby="remote-connect-group-bot"
            >
              <div
                id="remote-connect-bot-tabpanel"
                role="tabpanel"
                aria-labelledby={`remote-connect-bot-tab-${botTab}`}
              >
                {renderBotContent()}
              </div>
            </div>
          )}
        </div>
      </Modal>

      <Modal
        isOpen={isOpen && (disclaimerIsGate || showDisclaimer)}
        onClose={handleDisclaimerClose}
        title={t('remoteConnect.disclaimerTitle')}
        showCloseButton
        size="large"
        contentInset
      >
        <RemoteConnectDisclaimerContent
          agreed={hasAgreedDisclaimer}
          onClose={handleDisclaimerClose}
          onAgree={hasAgreedDisclaimer ? undefined : handleAgreeDisclaimer}
        />
      </Modal>

      {showRelayDeploy && (
        <RelayDeployWizard
          isOpen={showRelayDeploy}
          onClose={() => setShowRelayDeploy(false)}
          onRegistered={handleRelayDeployRegistered}
        />
      )}
    </>
  );
};

export default RemoteConnectDialog;
