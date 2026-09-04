/**
 * Device & Connections center.
 *
 * The overview keeps account-backed devices and account-free access in one
 * coherent place without presenting their different trust models as peer
 * modes. Detail views retain the complete existing capability set:
 *   - My devices (account, sync, and peer-device control)
 *   - Phone or browser (LAN / ngrok / OpenBitFun Relay / self-hosted)
 *   - Chat apps (Telegram / Feishu / WeChat)
 * Network and Chat Apps require an open workspace and can be active
 * simultaneously; My Devices works without a workspace.
 */

import {
  Button,
  Field,
  Icon,
  IconButton,
  Input,
  PageHeader,
  ScrollArea,
  Select,
  StatusPill,
  Switch,
  TabGroup,
  type TabGroupItem,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@openbitfun/ui';
import React, { useState, useEffect, useCallback, useRef } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { Monitor, MonitorSmartphone, Radar, Smartphone } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { getLocaleFallbackChain, type LocaleId } from '@/infrastructure/i18n/presets';
import { confirmWarning } from '@/infrastructure/confirm-dialog';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useAccountLoginState } from '@/infrastructure/account/useAccountLoginState';
import { useNotification } from '@/shared/notification-system';
import { copyTextToClipboard } from '@/shared/utils/textSelection';
import { AccountPanel } from './AccountPanel';
import {
  remoteConnectAPI,
  remotePairingStateName,
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
import { ChatAppBrandIcon } from './ChatAppBrandIcon';
import './RemoteConnectDialog.scss';

// ── Types ────────────────────────────────────────────────────────────

type ActiveGroup = 'network' | 'bot' | 'account';
type ActiveView = 'overview' | ActiveGroup;
type ConnectionOwner = Exclude<ActiveGroup, 'account'>;
type NetworkTab = 'lan' | 'ngrok' | 'openbitfun_server' | 'custom_server';
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
  { id: 'lan', labelKey: 'remoteConnect.methodSameNetwork' },
  { id: 'openbitfun_server', labelKey: 'remoteConnect.methodOpenBitFunRelay' },
  { id: 'ngrok', labelKey: 'remoteConnect.methodNgrok' },
  { id: 'custom_server', labelKey: 'remoteConnect.methodSelfHosted' },
];

const BOT_TABS: { id: BotTab; label: string }[] = [
  { id: 'telegram', label: 'Telegram' },
  { id: 'feishu', label: '' }, // filled from i18n
  { id: 'weixin', label: '' },
];

const NGROK_SETUP_URL = 'https://dashboard.ngrok.com/get-started/setup';
const FEISHU_SETUP_GUIDE_URLS = {
  'zh-CN': 'https://github.com/GCWing/OpenBitFun/blob/main/docs/remote-connect/feishu-bot-setup.zh-CN.md',
  'en-US': 'https://github.com/GCWing/OpenBitFun/blob/main/docs/remote-connect/feishu-bot-setup.md',
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
  if (method.startsWith('OpenBitFunServer')) return 'openbitfun_server';
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
   * Optional focused destination for contextual entry points. The generic
   * Device & Connections entry opens the overview.
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
  const {
    loggedIn: accountLoggedIn,
    deviceName: accountDeviceName,
  } = useAccountLoginState();

  const [activeView, setActiveView] = useState<ActiveView>(initialGroup ?? 'overview');
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
  const [weixinNeedsVerifyCode, setWeixinNeedsVerifyCode] = useState(false);
  const [weixinVerifyCode, setWeixinVerifyCode] = useState('');
  const [weixinQrPollNonce, setWeixinQrPollNonce] = useState(0);

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
  const weixinVerifyCodeRef = useRef<string | null>(null);
  const isOpenRef = useRef(isOpen);
  connectionOwnerRef.current = connectionOwner;
  isOpenRef.current = isOpen;

  // ── Derived state ────────────────────────────────────────────────

  const isRelayConnected = remotePairingStateName(status?.pairing_state) === 'connected';
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
    setWeixinNeedsVerifyCode(false);
    setWeixinVerifyCode('');
    weixinVerifyCodeRef.current = null;
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

  const handleViewChange = useCallback((nextView: ActiveView) => {
    if (nextView === activeView) return;
    if (activeView !== 'overview') void cancelPendingWork();
    setActiveView(nextView);
    setError(null);
  }, [activeView, cancelPendingWork]);

  useEffect(() => {
    if (!isOpen) return;
    setActiveView(initialGroup ?? 'overview');
    setError(null);
  }, [initialGroup, isOpen]);

  useEffect(() => {
    if (!isOpen) void cancelPendingWork();
  }, [cancelPendingWork, isOpen]);

  // ── Polling ──────────────────────────────────────────────────────

  const applyStatus = useCallback((nextStatus: RemoteConnectStatus) => {
    setStatus(nextStatus);

    // Relay and bot connections can coexist. Restore both selected subtabs
    // before choosing which group to show, otherwise the bot-first open path
    // can leave a connected OpenBitFun Server relay rendering the default LAN UI.
    if (remotePairingStateName(nextStatus.pairing_state) === 'connected') {
      const connectedTab = methodToNetworkTab(nextStatus.active_method);
      if (connectedTab) setNetworkTab(connectedTab);
    }
    const connectedBot = botInfoToBotTab(nextStatus.bot_connected);
    if (connectedBot) setBotTab(connectedBot);
  }, []);

  const startPolling = useCallback((target: 'relay' | 'bot') => {
    const pollGeneration = ++pollGenerationRef.current;
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      try {
        const s = await remoteConnectAPI.getStatus();
        if (!isOpenRef.current || pollGenerationRef.current !== pollGeneration) return;
        applyStatus(s);
        const done = target === 'relay'
          ? remotePairingStateName(s.pairing_state) === 'connected'
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
  }, [applyStatus]);

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
          applyStatus(s);
          setBotVerboseMode(s.bot_verbose_mode);

          if (['waiting_for_scan', 'verifying', 'handshaking'].includes(
            remotePairingStateName(s.pairing_state),
          )) {
            const tab = methodToNetworkTab(s.active_method);
            setActiveView('network');
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
  }, [applyStatus, isOpen, hasAgreedDisclaimer, startPolling]);

  useEffect(() => {
    if (!isOpen || !hasAgreedDisclaimer || activeView !== 'network' || networkTab !== 'lan') return;
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
  }, [isOpen, hasAgreedDisclaimer, activeView, networkTab]);

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
      let verifyCode = weixinVerifyCodeRef.current;
      weixinVerifyCodeRef.current = null;
      while (isCurrent()) {
        try {
          const p = await remoteConnectAPI.weixinQrPoll(key, null, verifyCode);
          verifyCode = null;
          if (!isCurrent()) return;
          if (p.status === 'scanned') {
            setWeixinQrImageUrl(null);
            setWeixinAwaitingPhoneConfirm(true);
            setWeixinNeedsVerifyCode(false);
            setWeixinVerifyCode('');
            await new Promise(resolve => setTimeout(resolve, 750));
            continue;
          }
          if (p.status === 'need_verify_code') {
            setWeixinQrImageUrl(null);
            setWeixinAwaitingPhoneConfirm(false);
            setWeixinNeedsVerifyCode(true);
            return;
          }
          if (p.status === 'confirmed' && p.ilink_token && p.bot_account_id) {
            const token = p.ilink_token;
            const base = p.base_url ?? '';
            const bid = p.bot_account_id;
            setWeixinAwaitingPhoneConfirm(false);
            setWeixinNeedsVerifyCode(false);
            setWeixinVerifyCode('');
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
            setWeixinNeedsVerifyCode(false);
            setWeixinVerifyCode('');
            return;
          }
          if (p.status === 'expired' && p.qr_image_url) {
            setWeixinQrImageUrl(p.qr_image_url);
            setWeixinAwaitingPhoneConfirm(false);
            setWeixinNeedsVerifyCode(false);
            setWeixinVerifyCode('');
          }
          await new Promise(resolve => setTimeout(resolve, 750));
        } catch (e: unknown) {
          updateIfOperationCurrent(isCurrent, () => {
            pendingOwnerRef.current = null;
            setError(e instanceof Error ? e.message : String(e));
            setWeixinQrSessionKey(null);
            setWeixinQrImageUrl(null);
            setWeixinAwaitingPhoneConfirm(false);
            setWeixinNeedsVerifyCode(false);
            setWeixinVerifyCode('');
          });
          return;
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [weixinQrSessionKey, weixinQrPollNonce, prepareAndStartWeixinBotFromQr, startPolling]);

  // ── Connection handlers ──────────────────────────────────────────

  const handleConnect = useCallback(async () => {
    if (!hasAgreedDisclaimer) {
      setShowDisclaimer(true);
      return;
    }
    if (activeView !== 'network' && activeView !== 'bot') return;
    const owner: ConnectionOwner = activeView;
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
      if (activeView === 'network' && networkTab === 'custom_server') {
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

      if (activeView === 'bot') {
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
  }, [activeView, networkTab, botTab, customUrl, tgToken, feishuAppId, feishuAppSecret, weixinIlinkToken, weixinBaseUrl, weixinBotAccountId, selectedLanIp, startPolling, t, hasAgreedDisclaimer]);

  const handleStartWeixinQr = useCallback(async () => {
    if (!hasAgreedDisclaimer) {
      setShowDisclaimer(true);
      return;
    }
    const operationGeneration = ++operationGenerationRef.current;
    pendingOwnerRef.current = 'bot';
    setError(null);
    setWeixinAwaitingPhoneConfirm(false);
    setWeixinNeedsVerifyCode(false);
    setWeixinVerifyCode('');
    weixinVerifyCodeRef.current = null;
    setLoading(true);
    try {
      await cleanupPromiseRef.current.catch(() => undefined);
      if (!isOpenRef.current || operationGenerationRef.current !== operationGeneration) return;
      const r = await remoteConnectAPI.weixinQrStart(
        weixinBaseUrl || null,
        weixinIlinkToken || null,
        weixinBotAccountId || null,
      );
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
  }, [hasAgreedDisclaimer, weixinBaseUrl, weixinBotAccountId, weixinIlinkToken]);

  const handleSubmitWeixinVerifyCode = useCallback(() => {
    const code = weixinVerifyCode.trim();
    if (!code || !weixinQrSessionKey) return;
    weixinVerifyCodeRef.current = code;
    setWeixinNeedsVerifyCode(false);
    setWeixinQrPollNonce(value => value + 1);
  }, [weixinQrSessionKey, weixinVerifyCode]);

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
      applyStatus(s);
    } catch { /* best effort */ }
  }, [applyStatus]);

  const handleDisconnectBot = useCallback(async () => {
    try {
      await remoteConnectAPI.stopBot();
      pendingOwnerRef.current = null;
      connectionOwnerRef.current = null;
      setConnectionOwner(null);
      setConnectionResult(null);
      const s = await remoteConnectAPI.getStatus();
      applyStatus(s);
    } catch { /* best effort */ }
  }, [applyStatus]);

  const handleBotVerboseModeChange = async (newMode: boolean) => {
    if (newMode === botVerboseMode) return;
    setBotVerboseMode(newMode);
    await remoteConnectAPI.setBotVerboseMode(newMode);
  };

  const handleCancelConnect = useCallback(async () => {
    await cancelPendingWork();
    if (!isOpenRef.current) return;
    try {
      const s = await remoteConnectAPI.getStatus();
      if (isOpenRef.current) applyStatus(s);
    } catch { /* best effort */ }
  }, [applyStatus, cancelPendingWork]);

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
    setActiveView('network');
    setError(null);
  }, []);

  const handleOpenFeishuGuide = useCallback(() => {
    void systemAPI.openExternal(pickLocalizedUrl(FEISHU_SETUP_GUIDE_URLS, currentLanguage));
  }, [currentLanguage]);

  const renderInfoCard = (children: React.ReactNode) => (
    <div className="openbitfun-remote-connect__info-card">
      {children}
    </div>
  );

  const renderSetupStep = (index: number, children: React.ReactNode) => (
    <p className="openbitfun-remote-connect__step">
      <span className="openbitfun-remote-connect__step-index" aria-hidden="true">{index}</span>
      <span>{children}</span>
    </p>
  );

  const botLabel = (tabId: BotTab | null): string | null => {
    if (tabId === 'telegram') return 'Telegram';
    if (tabId === 'feishu') return t('remoteConnect.feishu');
    if (tabId === 'weixin') return t('remoteConnect.weixin');
    return null;
  };

  const renderBotIdentity = () => {
    const label = botTab === 'telegram'
      ? 'Telegram'
      : botTab === 'feishu'
        ? t('remoteConnect.feishu')
        : t('remoteConnect.weixin');
    return (
      <div className="openbitfun-remote-connect__bot-identity">
        <span className="openbitfun-remote-connect__bot-identity-icon" aria-hidden="true">
          <ChatAppBrandIcon app={botTab} size={28} />
        </span>
        <h3 className="openbitfun-remote-connect__bot-identity-title">{label}</h3>
        <p className="openbitfun-remote-connect__bot-identity-description">
          {botTab === 'weixin'
            ? t('remoteConnect.botWeixinIntro')
            : t('remoteConnect.desc_bot')}
        </p>
      </div>
    );
  };

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
      <div data-openbitfun-component="remote-connect-dialog" data-openbitfun-part="error" className="openbitfun-remote-connect__error-group">
        <p className="openbitfun-remote-connect__error">{error}</p>
        {isNgrokErr && (
          <Button variant="outline" size="sm" onClick={handleOpenNgrokSetup}>
            {t('remoteConnect.openNgrokSetup')}
          </Button>
        )}
      </div>
    );
  };

  const renderConnectedView = (
    onDisconnect: () => void,
    username?: string | null,
  ) => (
    <div className="openbitfun-remote-connect__connected" data-openbitfun-component="remote-connect-dialog" data-openbitfun-part="body" data-openbitfun-state="connected">
      <div className="openbitfun-remote-connect__status" data-openbitfun-component="remote-connect-dialog" data-openbitfun-part="status" data-openbitfun-state="connected">
        <StatusPill tone="success">{t('remoteConnect.stateConnected')}</StatusPill>
        {username && (
          <span className="openbitfun-remote-connect__peer-username">
            {t('accountLogin.username')}: {username}
          </span>
        )}
      </div>
      <p className="openbitfun-remote-connect__hint">{t('remoteConnect.connectedHint')}</p>
      <Button variant="outline" size="md" onClick={onDisconnect}>
        {t('remoteConnect.disconnect')}
      </Button>
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
      <div
        data-openbitfun-component="remote-connect-dialog"
        data-openbitfun-part="body"
        className="openbitfun-remote-connect__body openbitfun-remote-connect__body--pairing"
      >
        <div
          className={`openbitfun-remote-connect__pairing-card${connectionResult.qr_url ? '' : ' openbitfun-remote-connect__pairing-card--compact'}`}
          data-openbitfun-component="remote-connect-dialog"
          data-openbitfun-part="pairingCard"
        >
          {(connectionResult.qr_url || connectionResult.bot_pairing_code) && (
            <div className="openbitfun-remote-connect__pairing-visual">
              {connectionResult.qr_url && (
                <button
                  type="button"
                  className="openbitfun-remote-connect__qr-box"
                  title={t('remoteConnect.copyUrl')}
                  aria-label={t('remoteConnect.copyUrl')}
                  onClick={() => void handleCopyPairingUrl()}
                >
                  <QRCodeSVG value={connectionResult.qr_url} size={180} level="M" includeMargin />
                </button>
              )}
              {connectionResult.bot_pairing_code && (
                <div className="openbitfun-remote-connect__pairing-code-box">
                  <div className="openbitfun-remote-connect__pairing-code">
                    {connectionResult.bot_pairing_code}
                  </div>
                </div>
              )}
              <StatusPill tone={qrCopied ? 'success' : 'warning'}>
                {qrCopied
                  ? t('remoteConnect.urlCopied')
                  : connectionOwner === 'bot'
                    ? t('remoteConnect.stateWaitingBot')
                    : t('remoteConnect.stateWaiting')}
              </StatusPill>
            </div>
          )}
          <div className="openbitfun-remote-connect__pairing-details">
            {connectionResult.qr_url && (
              <>
                <span className="openbitfun-remote-connect__pairing-label">
                  {t('remoteConnect.workspaceAddress')}
                </span>
                <div className="openbitfun-remote-connect__pairing-url-row">
                  <code title={connectionResult.qr_url}>{connectionResult.qr_url}</code>
                  <IconButton
                    aria-label={t('remoteConnect.copyUrl')}
                    title={t('remoteConnect.copyUrl')}
                    icon={qrCopied ? <Icon name="check-line" size="lg" /> : <Icon name="duplicate" size="lg" />}
                    onClick={() => void handleCopyPairingUrl()}
                    size="md"
                    variant="quiet"
                  />
                </div>
                <div className="openbitfun-remote-connect__pairing-instruction">
                  <Smartphone size={19} aria-hidden="true" />
                  <p>{t('remoteConnect.scanHint')}</p>
                </div>
                <div className="openbitfun-remote-connect__pairing-instruction">
                  <Icon name="browser" size="lg" aria-hidden="true" />
                  <p>{t('remoteConnect.mobileBrowserDescription')}</p>
                </div>
              </>
            )}
            {!connectionResult.qr_url && (
              <>
                <StatusPill tone="warning">
                  {connectionOwner === 'bot'
                    ? t('remoteConnect.stateWaitingBot')
                    : t('remoteConnect.stateWaiting')}
                </StatusPill>
                <p className="openbitfun-remote-connect__hint">
                  {connectionOwner === 'bot'
                    ? t('remoteConnect.botHint')
                    : t('remoteConnect.stateWaiting')}
                </p>
              </>
            )}
          </div>
        </div>
        <div className="openbitfun-remote-connect__pairing-actions">
          <Button variant="outline" size="md" onClick={handleCancelConnect}>
            {t('remoteConnect.cancel')}
          </Button>
        </div>
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
            <p className="openbitfun-remote-connect__ngrok-usage-link">
              <span
                className="openbitfun-remote-connect__description-link"
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
      <div
        data-openbitfun-component="remote-connect-dialog"
        data-openbitfun-part="body"
        className="openbitfun-remote-connect__body openbitfun-remote-connect__body--network"
      >
        {renderInfoCard(
          <>
            {networkTab === 'lan' && (lanNetworkInfo?.availableIps.length || lanNetworkInfo?.gatewayIp) && (
              <div className="openbitfun-remote-connect__info-meta-group">
                {lanNetworkInfo && lanNetworkInfo.availableIps.length > 0 && (
                  <div className="openbitfun-remote-connect__lan-ip-select">
                    <span className="openbitfun-remote-connect__info-meta-label">
                      {t('remoteConnect.currentIp')}
                    </span>
                    <Select
                      className="openbitfun-remote-connect__lan-ip-dropdown"
                      size="sm"
                      value={selectedLanIp}
                      onValueChange={(v) => setSelectedLanIp(String(v))}
                      options={lanNetworkInfo.availableIps.map(e => ({
                        label: `${e.ip} — ${e.interface_name}`,
                        value: e.ip,
                      }))}
                    />
                  </div>
                )}
                {(() => {
                  const selectedIntf = lanNetworkInfo?.availableIps.find(e => e.ip === selectedLanIp);
                  const gw = selectedIntf?.gateway_ip ?? null;
                  if (!gw) return null;
                  return (
                    <p className="openbitfun-remote-connect__info-meta">
                      {t('remoteConnect.gatewayIp')}: {gw}
                    </p>
                  );
                })()}
              </div>
            )}
            <p className="openbitfun-remote-connect__info-text">
              {networkTab === 'custom_server' ? (
                <>
                  {t('remoteConnect.desc_custom_server_prefix')}
                  <span
                    className="openbitfun-remote-connect__description-link"
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
                    className="openbitfun-remote-connect__description-link"
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
          <Field
            className="openbitfun-remote-connect__field openbitfun-remote-connect__field--inline"
            controlWidth="fill"
            label={t('remoteConnect.serverUrl')}
          >
            <Input
              className="openbitfun-remote-connect__input"
              type="url"
              placeholder="https://relay.example.com:9700"
              value={customUrl}
              onValueChange={setCustomUrl}
              size="md"
            />
          </Field>
        )}
        {renderErrorBlock()}
        <Button
          variant="fill"
          size="md"
          className="openbitfun-remote-connect__primary-action"
          loading={loading}
          onClick={handleConnect}
        >
          {loading ? t('remoteConnect.connecting') : t('remoteConnect.showConnectionCode')}
        </Button>
      </div>
    );
  };

  // ── Bot group content ────────────────────────────────────────────

  const renderBotContent = () => {
    if (isBotConnected && connectedBotTab === botTab) {
      const connectedLabel = botLabel(botTab) ?? botTab;
      const connectedDescription = t('remoteConnect.botConnectedDescription');
      return (
        <div
          data-openbitfun-component="remote-connect-dialog"
          data-openbitfun-part="body"
          data-openbitfun-state="connected"
          className="openbitfun-remote-connect__connected openbitfun-remote-connect__connected--bot"
        >
          <div className="openbitfun-remote-connect__connected-app">
            <span className="openbitfun-remote-connect__connected-app-icon" aria-hidden="true">
              <ChatAppBrandIcon app={botTab} size={25} />
            </span>
            <span className="openbitfun-remote-connect__connected-app-copy">
              <strong>{connectedLabel}</strong>
              <span>{connectedDescription}</span>
            </span>
            <div
              className="openbitfun-remote-connect__status"
              data-openbitfun-component="remote-connect-dialog"
              data-openbitfun-part="status"
              data-openbitfun-state="connected"
            >
              <StatusPill tone="success">{t('remoteConnect.stateConnected')}</StatusPill>
            </div>
          </div>
          {botTab === 'weixin' && (
            <div className="openbitfun-remote-connect__connected-notice">
              <Icon name="info" size="md" aria-hidden="true" />
              <p>{t('remoteConnect.botWeixinRestriction')}</p>
            </div>
          )}
          <div className="openbitfun-remote-connect__connected-setting">
            <div className="openbitfun-remote-connect__mode-setting">
              <span data-active={!botVerboseMode ? 'true' : undefined}>
                {t('remoteConnect.botConciseMode')}
              </span>
              <span className="openbitfun-remote-connect__mode-divider" aria-hidden="true">/</span>
              <span data-active={botVerboseMode ? 'true' : undefined}>
                {t('remoteConnect.botVerboseMode')}
              </span>
            </div>
            <Switch
              aria-label={`${t('remoteConnect.botConciseMode')} / ${t('remoteConnect.botVerboseMode')}`}
              checked={botVerboseMode}
              onCheckedChange={(checked) => void handleBotVerboseModeChange(checked)}
            />
          </div>
          <div className="openbitfun-remote-connect__connected-actions">
            <Button
              variant="outline"
              size="sm"
              onClick={handleDisconnectBot}
            >
              {t('remoteConnect.disconnect')}
            </Button>
          </div>
        </div>
      );
    }
    if (connectionResult && connectionOwner === 'bot') {
      return renderPairingInProgress();
    }
    return (
      <div
        data-openbitfun-component="remote-connect-dialog"
        data-openbitfun-part="body"
        className="openbitfun-remote-connect__body openbitfun-remote-connect__body--bot"
      >
        <div
          className="openbitfun-remote-connect__bot-card"
          data-openbitfun-component="remote-connect-dialog"
          data-openbitfun-part="botCard"
        >
          {renderBotIdentity()}
          <div className="openbitfun-remote-connect__bot-setup">
            {botTab === 'telegram' ? (
              <div className="openbitfun-remote-connect__bot-guide">
                {renderInfoCard(
                  <div className="openbitfun-remote-connect__steps">
                    {renderSetupStep(1, t('remoteConnect.botTgStep1'))}
                    {renderSetupStep(2, t('remoteConnect.botTgStep2'))}
                    {renderSetupStep(3, t('remoteConnect.botTgStep3'))}
                  </div>,
                )}
                <Field
                  className="openbitfun-remote-connect__field openbitfun-remote-connect__field--inline"
                  controlWidth="fill"
                  label="Bot Token"
                >
                  <Input
                    className="openbitfun-remote-connect__input"
                    type="text"
                    placeholder="123456:xxxxxxxxxxxxxxxxxxxxxxxx"
                    value={tgToken}
                    onValueChange={setTgToken}
                    size="md"
                  />
                </Field>
              </div>
            ) : botTab === 'feishu' ? (
              <div className="openbitfun-remote-connect__bot-guide">
                {renderInfoCard(
                  <>
                    <p className="openbitfun-remote-connect__info-text">
                      {t('remoteConnect.botFeishuDocPrefix')}
                      <span
                        className="openbitfun-remote-connect__description-link"
                        role="link"
                        tabIndex={0}
                        onClick={handleOpenFeishuGuide}
                        onKeyDown={(e) => { if (e.key === 'Enter') handleOpenFeishuGuide(); }}
                      >
                        {t('remoteConnect.botFeishuDocLink')}
                      </span>
                      {t('remoteConnect.botFeishuDocSuffix')}
                    </p>
                    <div className="openbitfun-remote-connect__steps">
                      {renderSetupStep(1, (
                        <>
                          {t('remoteConnect.botFeishuStep1Prefix')}
                          <span
                            className="openbitfun-remote-connect__step-link"
                            role="link"
                            tabIndex={0}
                            onClick={() => systemAPI.openExternal('https://open.feishu.cn/app')}
                            onKeyDown={(e) => { if (e.key === 'Enter') systemAPI.openExternal('https://open.feishu.cn/app'); }}
                          >
                            {t('remoteConnect.botFeishuOpenPlatform')}
                          </span>
                          {t('remoteConnect.botFeishuStep1Suffix')}
                        </>
                      ))}
                      {renderSetupStep(2, t('remoteConnect.botFeishuStep2'))}
                      {renderSetupStep(3, t('remoteConnect.botFeishuStep3'))}
                    </div>
                  </>,
                )}
                <Field
                  className="openbitfun-remote-connect__field openbitfun-remote-connect__field--inline"
                  controlWidth="fill"
                  label="App ID"
                >
                  <Input
                    className="openbitfun-remote-connect__input"
                    type="text"
                    placeholder="cli_xxxxxxxx"
                    value={feishuAppId}
                    onValueChange={setFeishuAppId}
                    size="md"
                  />
                </Field>
                <Field
                  className="openbitfun-remote-connect__field openbitfun-remote-connect__field--inline"
                  controlWidth="fill"
                  label="App Secret"
                >
                  <Input
                    className="openbitfun-remote-connect__input"
                    type="password"
                    placeholder="xxxxxxxxxxxxxxxx"
                    value={feishuAppSecret}
                    onValueChange={setFeishuAppSecret}
                    size="md"
                  />
                </Field>
              </div>
            ) : (
              <div className="openbitfun-remote-connect__bot-guide">
                {renderInfoCard(
                  <div className="openbitfun-remote-connect__steps">
                    {renderSetupStep(1, t('remoteConnect.botWeixinStep1'))}
                    {renderSetupStep(2, t('remoteConnect.botWeixinStep2'))}
                    <p className="openbitfun-remote-connect__info-text">
                      {t('remoteConnect.botWeixinRestriction')}
                    </p>
                  </div>,
                )}
                {weixinQrImageUrl && (
                  <div className="openbitfun-remote-connect__weixin-qr">
                    {isWeixinRasterQrSrc(weixinQrImageUrl) ? (
                      <img
                        src={weixinQrImageUrl}
                        alt={t('remoteConnect.weixinQrAlt')}
                        className="openbitfun-remote-connect__weixin-qr-img"
                      />
                    ) : (
                      <div
                        className="openbitfun-remote-connect__weixin-qr-svg-wrap"
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
                    <p className="openbitfun-remote-connect__hint">{t('remoteConnect.botWeixinPolling')}</p>
                    <Button variant="outline" size="md" onClick={handleCancelWeixinQr}>
                      {t('remoteConnect.botWeixinQrCancel')}
                    </Button>
                  </div>
                )}
                {weixinQrSessionKey && !weixinQrImageUrl && weixinAwaitingPhoneConfirm && (
                  <div className="openbitfun-remote-connect__weixin-qr openbitfun-remote-connect__weixin-qr--await">
                    <p className="openbitfun-remote-connect__hint">{t('remoteConnect.botWeixinAwaitingPhoneConfirm')}</p>
                    <Button variant="outline" size="md" onClick={handleCancelWeixinQr}>
                      {t('remoteConnect.botWeixinQrCancel')}
                    </Button>
                  </div>
                )}
                {weixinQrSessionKey && !weixinQrImageUrl && weixinNeedsVerifyCode && (
                  <div className="openbitfun-remote-connect__weixin-verify">
                    <Input
                      className="openbitfun-remote-connect__field openbitfun-remote-connect__field--inline"
                      type="text"
                      inputMode="numeric"
                      autoComplete="one-time-code"
                      aria-label={t('remoteConnect.botWeixinVerifyCodePlaceholder')}
                      placeholder={t('remoteConnect.botWeixinVerifyCodePlaceholder')}
                      value={weixinVerifyCode}
                      onValueChange={setWeixinVerifyCode}
                      size="md"
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') handleSubmitWeixinVerifyCode();
                      }}
                    />
                    <p className="openbitfun-remote-connect__hint">
                      {t('remoteConnect.botWeixinVerifyCodeHint')}
                    </p>
                    <Button
                      variant="fill"
                      size="md"
                      className="openbitfun-remote-connect__primary-action"
                      onClick={handleSubmitWeixinVerifyCode}
                      disabled={!weixinVerifyCode.trim()}
                    >
                      {t('remoteConnect.botWeixinVerifyCodeSubmit')}
                    </Button>
                  </div>
                )}
                {!weixinQrSessionKey && !weixinQrImageUrl && !weixinNeedsVerifyCode && (
                  <Button
                    variant="fill"
                    size="md"
                    className="openbitfun-remote-connect__primary-action"
                    loading={loading}
                    onClick={handleStartWeixinQr}
                  >
                    {t('remoteConnect.botWeixinQrButton')}
                  </Button>
                )}
              </div>
            )}
            {renderErrorBlock()}
            {botTab !== 'weixin' && (
              <Button
                variant="fill"
                size="md"
                className="openbitfun-remote-connect__primary-action"
                loading={loading}
                onClick={handleConnect}
                disabled={botTab === 'telegram' ? !tgToken : !feishuAppId}
              >
                {loading ? t('remoteConnect.connecting') : t('remoteConnect.getPairingCode')}
              </Button>
            )}
          </div>
        </div>
      </div>
    );
  };

  // ── Layout ───────────────────────────────────────────────────────

  const isNetworkConnecting = !!connectionResult && connectionOwner === 'network' && !isRelayConnected;
  const isBotConnecting = !!connectionResult && connectionOwner === 'bot' && !isBotConnected;
  const isCurrentViewPairing = activeView === 'network'
    ? isNetworkConnecting || (loading && pendingOwnerRef.current === 'network')
    : activeView === 'bot'
      ? isBotConnecting || !!weixinQrSessionKey || (loading && pendingOwnerRef.current === 'bot')
      : false;

  const networkLabel = (tabId: NetworkTab | null): string | null => {
    const tab = NETWORK_TABS.find(item => item.id === tabId);
    return tab ? t(tab.labelKey) : null;
  };

  const handleAgreeDisclaimer = useCallback(() => {
    setRemoteConnectDisclaimerAgreed();
    setHasAgreedDisclaimer(true);
    setShowDisclaimer(false);
  }, []);

  useEffect(() => {
    if (
      isOpen
      && hasAgreedDisclaimer
      && !hasWorkspace
      && (activeView === 'network' || activeView === 'bot')
    ) {
      handleViewChange('overview');
    }
  }, [activeView, handleViewChange, hasAgreedDisclaimer, hasWorkspace, isOpen]);

  const renderOverviewAction = ({
    view,
    icon,
    title,
    description,
    statusLabel,
    statusDetail,
    statusPositive = false,
    state,
    disabled = false,
  }: {
    view: ActiveGroup;
    icon: React.ReactNode;
    title: string;
    description: string;
    statusLabel: string;
    statusDetail?: string | null;
    statusPositive?: boolean;
    state?: 'authenticated' | 'connected';
    disabled?: boolean;
  }) => (
    <button
      type="button"
      className="openbitfun-remote-connect__overview-action"
      data-openbitfun-component="remote-connect-dialog"
      data-openbitfun-part="overviewAction"
      data-openbitfun-group={view}
      data-openbitfun-state={[
        state,
        disabled && 'disabled',
      ].filter(Boolean).join(' ') || undefined}
      onClick={() => handleViewChange(view)}
      disabled={disabled}
    >
      <span className="openbitfun-remote-connect__overview-action-icon" aria-hidden="true">
        {icon}
      </span>
      <span className="openbitfun-remote-connect__overview-action-copy">
        <span className="openbitfun-remote-connect__overview-action-title">{title}</span>
        <span className="openbitfun-remote-connect__overview-action-description">{description}</span>
      </span>
      <span className="openbitfun-remote-connect__overview-action-status">
        {statusDetail && (
          <span className="openbitfun-remote-connect__overview-action-status-detail">
            {statusDetail}
          </span>
        )}
        <StatusPill tone={statusPositive ? 'success' : 'neutral'}>{statusLabel}</StatusPill>
      </span>
      <Icon name="chevron-right" size="md" className="openbitfun-remote-connect__overview-action-chevron" aria-hidden="true" />
    </button>
  );

  const renderOverview = () => (
    <ScrollArea
      className="openbitfun-remote-connect__overview"
      data-openbitfun-component="remote-connect-dialog"
      data-openbitfun-part="overview"
    >
      <section
        className="openbitfun-remote-connect__overview-section"
        data-openbitfun-component="remote-connect-dialog"
        data-openbitfun-part="overviewSection"
        aria-labelledby="remote-connect-my-devices-title"
      >
        <h2 id="remote-connect-my-devices-title" className="openbitfun-remote-connect__overview-section-title">
          <span
            className="openbitfun-remote-connect__section-marker openbitfun-remote-connect__section-marker--account"
            data-openbitfun-component="remote-connect-dialog"
            data-openbitfun-part="sectionMarker"
            aria-hidden="true"
          />
          {t('remoteConnect.myDevicesTitle')}
        </h2>
        <div className="openbitfun-remote-connect__overview-actions openbitfun-remote-connect__overview-actions--account">
          {renderOverviewAction({
            view: 'account',
            icon: <Monitor size={18} />,
            title: t('remoteConnect.accountDevicesTitle'),
            description: t('remoteConnect.myDevicesDescription'),
            statusLabel: accountLoggedIn
              ? t('remoteConnect.accountSignedIn')
              : t('remoteConnect.accountSignedOut'),
            statusDetail: accountLoggedIn ? accountDeviceName : null,
            statusPositive: accountLoggedIn,
            state: accountLoggedIn ? 'authenticated' : undefined,
          })}
        </div>
      </section>

      <section
        className="openbitfun-remote-connect__overview-section"
        data-openbitfun-component="remote-connect-dialog"
        data-openbitfun-part="overviewSection"
        aria-labelledby="remote-connect-access-title"
      >
        <div className="openbitfun-remote-connect__overview-section-heading">
          <h2 id="remote-connect-access-title" className="openbitfun-remote-connect__overview-section-title">
            <span
              className="openbitfun-remote-connect__section-marker"
              data-openbitfun-component="remote-connect-dialog"
              data-openbitfun-part="sectionMarker"
              aria-hidden="true"
            />
            {t('remoteConnect.connectThisDeviceTitle')}
          </h2>
          <p className="openbitfun-remote-connect__overview-section-description">
            {t('remoteConnect.connectThisDeviceDescription')}
          </p>
        </div>
        <div className="openbitfun-remote-connect__overview-actions openbitfun-remote-connect__overview-actions--access">
          {renderOverviewAction({
            view: 'network',
            icon: <Smartphone size={18} />,
            title: t('remoteConnect.mobileBrowserTitle'),
            description: t('remoteConnect.mobileBrowserDescription'),
            statusLabel: !hasWorkspace
              ? t('remoteConnect.requiresWorkspace')
              : isRelayConnected
                ? t('remoteConnect.stateConnected')
                : t('remoteConnect.notConnected'),
            statusDetail: hasWorkspace && isRelayConnected
              ? networkLabel(connectedNetworkTab)
              : null,
            statusPositive: hasWorkspace && isRelayConnected,
            state: hasWorkspace && isRelayConnected ? 'connected' : undefined,
            disabled: !hasWorkspace,
          })}
          {renderOverviewAction({
            view: 'bot',
            icon: (
              <span className="openbitfun-remote-connect__chat-brand-group">
                {BOT_TABS.map(tab => (
                  <span
                    className="openbitfun-remote-connect__chat-brand-item"
                    data-connected={isBotConnected && connectedBotTab === tab.id ? 'true' : undefined}
                    key={tab.id}
                  >
                    <ChatAppBrandIcon app={tab.id} size={15} />
                  </span>
                ))}
              </span>
            ),
            title: t('remoteConnect.chatAppsTitle'),
            description: t('remoteConnect.chatAppsDescription'),
            statusLabel: !hasWorkspace
              ? t('remoteConnect.requiresWorkspace')
              : isBotConnected
                ? t('remoteConnect.stateConnected')
                : t('remoteConnect.notConnected'),
            statusDetail: hasWorkspace && isBotConnected
              ? botLabel(connectedBotTab)
              : null,
            statusPositive: hasWorkspace && isBotConnected,
            state: hasWorkspace && isBotConnected ? 'connected' : undefined,
            disabled: !hasWorkspace,
          })}
        </div>
      </section>
    </ScrollArea>
  );

  const renderViewHeader = () => {
    if (activeView === 'overview') return null;
    const title = activeView === 'account'
      ? t('remoteConnect.myDevicesTitle')
      : activeView === 'network'
        ? t('remoteConnect.mobileBrowserTitle')
        : t('remoteConnect.chatAppsTitle');
    const description = activeView === 'account'
      ? t('remoteConnect.myDevicesDescription')
      : activeView === 'network'
        ? t('remoteConnect.mobileBrowserDescription')
        : t('remoteConnect.chatAppsDescription');

    return (
      <div
        className="openbitfun-remote-connect__view-header"
        data-openbitfun-component="remote-connect-dialog"
        data-openbitfun-part="viewHeader"
      >
        <Button
          className="openbitfun-remote-connect__back"
          leadingIcon={<Icon name="arrow-left" size="lg" />}
          onClick={() => handleViewChange('overview')}
          size="sm"
          variant="text"
        >
          {isCurrentViewPairing
            ? t('remoteConnect.cancelAndBack')
            : t('remoteConnect.backToOverview')}
        </Button>
        <PageHeader
          className="openbitfun-remote-connect__view-page-header"
          description={description}
          level={2}
          size="lg"
          title={<span id="remote-connect-view-title">{title}</span>}
        />
      </div>
    );
  };

  const renderConnectionTabLabel = (
    label: string,
    connected: boolean,
    brand?: BotTab,
  ) => (
    <span className="openbitfun-remote-connect__tab-label">
      {brand && (
        <span className="openbitfun-remote-connect__tab-brand" aria-hidden="true">
          <ChatAppBrandIcon app={brand} size={15} />
        </span>
      )}
      <span>{label}</span>
      {connected && <span className="openbitfun-remote-connect__dot-sm" aria-hidden="true" />}
      {connected && (
        <span className="openbitfun-remote-connect__visually-hidden">
          {` · ${t('remoteConnect.stateConnected')}`}
        </span>
      )}
    </span>
  );

  const networkTabItems: TabGroupItem[] = NETWORK_TABS.map(tab => ({
    disabled: isNetworkSubDisabled(tab.id) || (isNetworkConnecting && networkTab !== tab.id),
    id: `remote-connect-network-tab-${tab.id}`,
    label: renderConnectionTabLabel(
      t(tab.labelKey),
      isRelayConnected && connectedNetworkTab === tab.id,
    ),
    panelId: 'remote-connect-network-tabpanel',
    value: tab.id,
  }));
  const botTabItems: TabGroupItem[] = BOT_TABS.map(tab => ({
    disabled: isBotSubDisabled(tab.id) || (isBotConnecting && botTab !== tab.id),
    id: `remote-connect-bot-tab-${tab.id}`,
    label: renderConnectionTabLabel(
      botLabel(tab.id) ?? tab.label,
      isBotConnected && connectedBotTab === tab.id,
      tab.id,
    ),
    panelId: 'remote-connect-bot-tabpanel',
    value: tab.id,
  }));

  const handleNetworkTabValueChange = (value: string) => {
    const nextTab = value as NetworkTab;
    if (nextTab === networkTab) return;
    void cancelPendingWork();
    setNetworkTab(nextTab);
    setError(null);
  };

  const handleBotTabValueChange = (value: string) => {
    const nextTab = value as BotTab;
    if (nextTab === botTab) return;
    void cancelPendingWork();
    setBotTab(nextTab);
    setError(null);
  };

  const disclaimerIsGate = isOpen && !hasAgreedDisclaimer;
  const handleDisclaimerClose = disclaimerIsGate
    ? handleDialogClose
    : () => setShowDisclaimer(false);

  return (
    <>
      <Dialog
        open={isOpen && hasAgreedDisclaimer}
        onOpenChange={(nextOpen) => { if (!nextOpen) handleDialogClose(); }}
        size="2xl"
        aria-label={t('remoteConnect.centerTitle')}
        className="openbitfun-remote-connect-dialog"
      >
        <DialogHeader className="openbitfun-remote-connect-dialog__header">
          <DialogClose />
        </DialogHeader>
        <DialogBody className="openbitfun-remote-connect-dialog__body" inset="none">
          <div
            className="openbitfun-remote-connect"
            data-openbitfun-component="remote-connect-dialog"
            data-openbitfun-part="root"
            data-openbitfun-view={activeView}
          >
          <aside
            className="openbitfun-remote-connect__sidebar"
            data-openbitfun-component="remote-connect-dialog"
            data-openbitfun-part="sidebar"
          >
            <div
              className="openbitfun-remote-connect__sidebar-brand"
              data-openbitfun-component="remote-connect-dialog"
              data-openbitfun-part="sidebarBrand"
            >
              <span className="openbitfun-remote-connect__sidebar-icon" aria-hidden="true">
                <MonitorSmartphone size={34} strokeWidth={1.65} />
              </span>
              <h2 id="remote-connect-center-title" className="openbitfun-remote-connect__sidebar-title">
                {t('remoteConnect.centerTitle')}
              </h2>
              <span className="openbitfun-remote-connect__title-extra">
                <Button
                  className="openbitfun-remote-connect__disclaimer-trigger"
                  onClick={() => setShowDisclaimer(true)}
                  size="sm"
                  variant="text"
                >
                  {t('remoteConnect.disclaimerReview')}
                </Button>
              </span>
              <span className="openbitfun-remote-connect__sidebar-divider" aria-hidden="true" />
              <p className="openbitfun-remote-connect__sidebar-description">
                {t('remoteConnect.overviewIntro')}
              </p>
            </div>
            <Radar
              className="openbitfun-remote-connect__sidebar-decoration"
              size={300}
              strokeWidth={0.55}
              aria-hidden="true"
            />
          </aside>

          <main
            className="openbitfun-remote-connect__main"
            data-openbitfun-component="remote-connect-dialog"
            data-openbitfun-part="main"
            aria-labelledby="remote-connect-center-title"
          >
            {activeView === 'overview' ? renderOverview() : (
              <>
                {renderViewHeader()}

                {activeView === 'network' ? (
                  <div
                    className="openbitfun-remote-connect__subtabs"
                    data-openbitfun-component="remote-connect-dialog"
                    data-openbitfun-part="subtabs"
                    data-openbitfun-group="network"
                  >
                    <TabGroup
                      aria-label={t('remoteConnect.mobileBrowserTitle')}
                      className="openbitfun-remote-connect__tab-group"
                      items={networkTabItems}
                      onValueChange={handleNetworkTabValueChange}
                      value={networkTab}
                    />
                  </div>
                ) : activeView === 'bot' ? (
                  <div
                    className="openbitfun-remote-connect__subtabs"
                    data-openbitfun-component="remote-connect-dialog"
                    data-openbitfun-part="subtabs"
                    data-openbitfun-group="bot"
                  >
                    <TabGroup
                      aria-label={t('remoteConnect.chatAppsTitle')}
                      className="openbitfun-remote-connect__tab-group"
                      items={botTabItems}
                      onValueChange={handleBotTabValueChange}
                      value={botTab}
                    />
                  </div>
                ) : null}

                {activeView === 'account' ? (
                  <div
                    id="remote-connect-panel-account"
                    data-openbitfun-component="remote-connect-dialog"
                    data-openbitfun-part="panel"
                    data-openbitfun-group="account"
                    role="region"
                    aria-labelledby="remote-connect-view-title"
                  >
                    <AccountPanel onCloseDialog={handleDialogClose} />
                  </div>
                ) : activeView === 'network' ? (
                  <ScrollArea
                    id="remote-connect-panel-network"
                    data-openbitfun-component="remote-connect-dialog"
                    data-openbitfun-part="panel"
                    data-openbitfun-group="network"
                  >
                    <div
                      id="remote-connect-network-tabpanel"
                      role="tabpanel"
                      aria-labelledby={`remote-connect-network-tab-${networkTab}`}
                    >
                      {renderNetworkContent()}
                    </div>
                  </ScrollArea>
                ) : (
                  <ScrollArea
                    id="remote-connect-panel-bot"
                    data-openbitfun-component="remote-connect-dialog"
                    data-openbitfun-part="panel"
                    data-openbitfun-group="bot"
                  >
                    <div
                      id="remote-connect-bot-tabpanel"
                      role="tabpanel"
                      aria-labelledby={`remote-connect-bot-tab-${botTab}`}
                    >
                      {renderBotContent()}
                    </div>
                  </ScrollArea>
                )}
              </>
            )}
          </main>
          </div>
        </DialogBody>
      </Dialog>

      <Dialog
        open={isOpen && (disclaimerIsGate || showDisclaimer)}
        onOpenChange={(nextOpen) => { if (!nextOpen) handleDisclaimerClose(); }}
        size="lg"
      >
        <DialogHeader>
          <DialogHeading>
            <DialogTitle>{t('remoteConnect.disclaimerTitle')}</DialogTitle>
          </DialogHeading>
          <DialogClose />
        </DialogHeader>
        <DialogBody>
        <RemoteConnectDisclaimerContent
          agreed={hasAgreedDisclaimer}
          onClose={handleDisclaimerClose}
          onAgree={hasAgreedDisclaimer ? undefined : handleAgreeDisclaimer}
        />
              </DialogBody>
      </Dialog>

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
