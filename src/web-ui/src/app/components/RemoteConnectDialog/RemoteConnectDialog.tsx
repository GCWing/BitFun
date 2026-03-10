/**
 * Remote Connect dialog with two independent groups:
 *   - Network (LAN / Ngrok / BitFun Server / Custom Server) – mutually exclusive
 *   - SMS Bot (Telegram / Feishu) – mutually exclusive
 * Both groups can be active simultaneously.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal, Badge, Input } from '@/component-library';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import {
  remoteConnectAPI,
  type ConnectionResult,
  type RemoteConnectStatus,
} from '@/infrastructure/api/service-api/RemoteConnectAPI';
import {
  getRemoteConnectDisclaimerAgreed,
  setRemoteConnectDisclaimerAgreed,
  RemoteConnectDisclaimerContent,
} from './RemoteConnectDisclaimer';
import './RemoteConnectDialog.scss';

// ── Types ────────────────────────────────────────────────────────────

type ActiveGroup = 'network' | 'bot';
type NetworkTab = 'lan' | 'ngrok' | 'custom_server';
type BotTab = 'telegram' | 'feishu';

const NETWORK_TABS: { id: NetworkTab; labelKey: string }[] = [
  { id: 'lan', labelKey: 'remoteConnect.tabLan' },
  { id: 'ngrok', labelKey: 'remoteConnect.tabNgrok' },
  { id: 'custom_server', labelKey: 'remoteConnect.tabCustomServer' },
];

const BOT_TABS: { id: BotTab; label: string }[] = [
  { id: 'telegram', label: 'Telegram' },
  { id: 'feishu', label: '' }, // filled from i18n
];

const NGROK_SETUP_URL = 'https://dashboard.ngrok.com/get-started/setup';
const RELAY_SERVER_README_URL = 'https://github.com/GCWing/BitFun/blob/main/src/apps/relay-server/README.md';
const FEISHU_SETUP_GUIDE_URLS = {
  'zh-CN': 'https://github.com/GCWing/BitFun/blob/main/docs/remote-connect/feishu-bot-setup.zh-CN.md',
  'en-US': 'https://github.com/GCWing/BitFun/blob/main/docs/remote-connect/feishu-bot-setup.md',
} as const;

const methodToNetworkTab = (method: string | null | undefined): NetworkTab | null => {
  if (!method) return null;
  if (method.startsWith('Lan')) return 'lan';
  if (method.startsWith('Ngrok')) return 'ngrok';
  if (method.startsWith('CustomServer')) return 'custom_server';
  return null;
};

const botInfoToBotTab = (info: string | null | undefined): BotTab | null => {
  if (!info) return null;
  if (info.startsWith('Telegram')) return 'telegram';
  if (info.startsWith('Feishu')) return 'feishu';
  return null;
};

// ── Component ────────────────────────────────────────────────────────

interface RemoteConnectDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export const RemoteConnectDialog: React.FC<RemoteConnectDialogProps> = ({
  isOpen,
  onClose,
}) => {
  const { t, currentLanguage } = useI18n('common');

  const [activeGroup, setActiveGroup] = useState<ActiveGroup>('network');
  const [networkTab, setNetworkTab] = useState<NetworkTab>(NETWORK_TABS[0].id);
  const [botTab, setBotTab] = useState<BotTab>(BOT_TABS[0].id);

  const [connectionResult, setConnectionResult] = useState<ConnectionResult | null>(null);
  const [status, setStatus] = useState<RemoteConnectStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lanNetworkInfo, setLanNetworkInfo] = useState<{ localIp: string; gatewayIp: string | null } | null>(null);
  const [showDisclaimer, setShowDisclaimer] = useState(false);
  const [hasAgreedDisclaimer, setHasAgreedDisclaimer] = useState<boolean>(() => getRemoteConnectDisclaimerAgreed());

  const [qrCopied, setQrCopied] = useState(false);
  const [customUrl, setCustomUrl] = useState('');
  const [tgToken, setTgToken] = useState('');
  const [feishuAppId, setFeishuAppId] = useState('');
  const [feishuAppSecret, setFeishuAppSecret] = useState('');

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const pollTargetRef = useRef<'relay' | 'bot'>('relay');

  // ── Derived state ────────────────────────────────────────────────

  const isRelayConnected = status?.pairing_state === 'connected';
  const isBotConnected = !!status?.bot_connected;
  const connectedNetworkTab = methodToNetworkTab(status?.active_method);
  const connectedBotTab = botInfoToBotTab(status?.bot_connected);

  // ── Polling ──────────────────────────────────────────────────────

  const startPolling = useCallback((target: 'relay' | 'bot') => {
    pollTargetRef.current = target;
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      try {
        const s = await remoteConnectAPI.getStatus();
        setStatus(s);
        const done = target === 'relay'
          ? s.pairing_state === 'connected'
          : !!s.bot_connected;
        if (done) {
          if (pollRef.current) clearInterval(pollRef.current);
          pollRef.current = null;
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

    setHasAgreedDisclaimer(getRemoteConnectDisclaimerAgreed());

    let cancelled = false;
    const checkExisting = async () => {
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          const s = await remoteConnectAPI.getStatus();
          if (cancelled) return;
          setStatus(s);

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
    };
  }, [isOpen, startPolling]);

  useEffect(() => {
    if (!isOpen || activeGroup !== 'network' || networkTab !== 'lan') return;
    let cancelled = false;
    const loadLanNetworkInfo = async () => {
      const info = await remoteConnectAPI.getLanNetworkInfo();
      if (!cancelled) {
        setLanNetworkInfo(
          info
            ? { localIp: info.local_ip, gatewayIp: info.gateway_ip ?? null }
            : null,
        );
      }
    };
    void loadLanNetworkInfo();
    return () => {
      cancelled = true;
    };
  }, [isOpen, activeGroup, networkTab]);

  // ── Connection handlers ──────────────────────────────────────────

  const handleConnect = useCallback(async () => {
    setLoading(true);
    setError(null);
    setConnectionResult(null);

    try {
      let method: string;
      let serverUrl: string | undefined;

      if (activeGroup === 'bot') {
        method = botTab === 'telegram' ? 'bot_telegram' : 'bot_feishu';
        if (botTab === 'telegram' && tgToken) {
          await remoteConnectAPI.configureBot({ botType: 'telegram', botToken: tgToken });
        } else if (botTab === 'feishu' && feishuAppId) {
          await remoteConnectAPI.configureBot({
            botType: 'feishu', appId: feishuAppId, appSecret: feishuAppSecret,
          });
        }
      } else {
        method = networkTab;
        if (networkTab === 'custom_server') serverUrl = customUrl || undefined;
      }

      const result = await remoteConnectAPI.startConnection(method, serverUrl);
      setConnectionResult(result);
      startPolling(activeGroup === 'bot' ? 'bot' : 'relay');
    } catch (e: any) {
      setError(e?.message || String(e));
    } finally {
      setLoading(false);
    }
  }, [activeGroup, networkTab, botTab, customUrl, tgToken, feishuAppId, feishuAppSecret, startPolling]);

  const handleDisconnectRelay = useCallback(async () => {
    try {
      await remoteConnectAPI.stopConnection();
      setConnectionResult(null);
      const s = await remoteConnectAPI.getStatus();
      setStatus(s);
    } catch { /* best effort */ }
  }, []);

  const handleDisconnectBot = useCallback(async () => {
    try {
      await remoteConnectAPI.stopBot();
      setConnectionResult(null);
      const s = await remoteConnectAPI.getStatus();
      setStatus(s);
    } catch { /* best effort */ }
  }, []);

  const handleCancelConnect = useCallback(async () => {
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = null;
    try {
      if (activeGroup === 'bot') {
        await remoteConnectAPI.stopBot();
      } else {
        await remoteConnectAPI.stopConnection();
      }
    } catch { /* best effort */ }
    setConnectionResult(null);
    const s = await remoteConnectAPI.getStatus();
    setStatus(s);
  }, [activeGroup]);

  const handleOpenNgrokSetup = useCallback(() => {
    void systemAPI.openExternal(NGROK_SETUP_URL);
  }, []);

  const handleOpenRelayReadme = useCallback(() => {
    void systemAPI.openExternal(RELAY_SERVER_README_URL);
  }, []);

  const handleOpenFeishuGuide = useCallback(() => {
    void systemAPI.openExternal(FEISHU_SETUP_GUIDE_URLS[currentLanguage]);
  }, [currentLanguage]);

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

  const renderConnectedView = (peerName: string, onDisconnect: () => void) => (
    <div className="bitfun-remote-connect__connected">
      <div className="bitfun-remote-connect__status">
        <Badge variant="success">{t('remoteConnect.stateConnected')}</Badge>
        <span className="bitfun-remote-connect__peer-name">{peerName}</span>
      </div>
      <p className="bitfun-remote-connect__hint">{t('remoteConnect.connectedHint')}</p>
      <button type="button" className="bitfun-remote-connect__btn bitfun-remote-connect__btn--disconnect" onClick={onDisconnect}>
        {t('remoteConnect.disconnect')}
      </button>
    </div>
  );

  const renderPairingInProgress = () => {
    if (!connectionResult) return null;
    return (
      <div className="bitfun-remote-connect__body">
        {connectionResult.qr_url && (
          <div
            className="bitfun-remote-connect__qr-box"
            style={{ cursor: 'pointer' }}
            title="Click to copy URL"
            onClick={() => {
              navigator.clipboard.writeText(connectionResult.qr_url!);
              setQrCopied(true);
              setTimeout(() => setQrCopied(false), 2000);
            }}
          >
            <QRCodeSVG value={connectionResult.qr_url} size={180} level="M" includeMargin />
          </div>
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
              : activeGroup === 'bot'
                ? t('remoteConnect.stateWaitingBot')
                : t('remoteConnect.stateWaiting')}
          </Badge>
        </div>
        <p className="bitfun-remote-connect__hint">
          {activeGroup === 'bot' ? t('remoteConnect.botHint') : t('remoteConnect.scanHint')}
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
            status?.peer_device_name ?? t('remoteConnect.stateConnected'),
            handleDisconnectRelay,
          )}
        </>
      );
    }
    if (connectionResult && activeGroup === 'network') {
      return renderPairingInProgress();
    }
    return (
      <div className="bitfun-remote-connect__body">
        {networkTab === 'lan' && lanNetworkInfo?.localIp && (
          <p className="bitfun-remote-connect__hint">
            {t('remoteConnect.currentIp')}: {lanNetworkInfo.localIp}
          </p>
        )}
        {networkTab === 'lan' && lanNetworkInfo?.gatewayIp && (
          <p className="bitfun-remote-connect__hint">
            {t('remoteConnect.gatewayIp')}: {lanNetworkInfo.gatewayIp}
          </p>
        )}
        <p className="bitfun-remote-connect__description">
          {networkTab === 'custom_server' ? (
            <>
              {t('remoteConnect.desc_custom_server_prefix')}
              <span
                className="bitfun-remote-connect__description-link"
                role="link"
                tabIndex={0}
                onClick={handleOpenRelayReadme}
                onKeyDown={(e) => { if (e.key === 'Enter') handleOpenRelayReadme(); }}
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
        {networkTab === 'custom_server' && (
          <Input
            className="bitfun-remote-connect__field"
            label={t('remoteConnect.serverUrl')}
            type="url"
            placeholder="https://relay.example.com:9700"
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
      return renderConnectedView(status!.bot_connected!, handleDisconnectBot);
    }
    if (connectionResult && activeGroup === 'bot') {
      return renderPairingInProgress();
    }
    return (
      <div className="bitfun-remote-connect__body">
        {botTab === 'telegram' ? (
          <div className="bitfun-remote-connect__bot-guide">
            <div className="bitfun-remote-connect__steps">
              <p className="bitfun-remote-connect__step">1. {t('remoteConnect.botTgStep1')}</p>
              <p className="bitfun-remote-connect__step">2. {t('remoteConnect.botTgStep2')}</p>
              <p className="bitfun-remote-connect__step">3. {t('remoteConnect.botTgStep3')}</p>
            </div>
            <Input
              className="bitfun-remote-connect__field"
              label="Bot Token"
              type="text"
              placeholder="123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"
              value={tgToken}
              onChange={(e) => setTgToken(e.target.value)}
            />
          </div>
        ) : (
          <div className="bitfun-remote-connect__bot-guide">
            <p className="bitfun-remote-connect__description">
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
        )}
        {renderErrorBlock()}
        <button
          type="button"
          className="bitfun-remote-connect__btn bitfun-remote-connect__btn--connect"
          onClick={handleConnect}
          disabled={loading || (botTab === 'telegram' ? !tgToken : !feishuAppId)}
        >
          {loading ? t('remoteConnect.connecting') : t('remoteConnect.connect')}
        </button>
      </div>
    );
  };

  // ── Layout ───────────────────────────────────────────────────────

  const isNetworkConnecting = !!connectionResult && activeGroup === 'network' && !isRelayConnected;
  const isBotConnecting = !!connectionResult && activeGroup === 'bot' && !isBotConnected;
  const handleAgreeDisclaimer = useCallback(() => {
    setRemoteConnectDisclaimerAgreed();
    setHasAgreedDisclaimer(true);
    setShowDisclaimer(false);
  }, []);

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={onClose}
        title={t('remoteConnect.title')}
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
          <div className="bitfun-remote-connect__groups">
            <button
              type="button"
              className={`bitfun-remote-connect__group-btn${activeGroup === 'network' ? ' is-active' : ''}`}
              onClick={() => { setActiveGroup('network'); setConnectionResult(null); setError(null); }}
              disabled={isBotConnecting}
            >
              {t('remoteConnect.groupNetwork')}
              {isRelayConnected && <span className="bitfun-remote-connect__dot" />}
            </button>
            <span className="bitfun-remote-connect__group-divider" />
            <button
              type="button"
              className={`bitfun-remote-connect__group-btn${activeGroup === 'bot' ? ' is-active' : ''}`}
              onClick={() => { setActiveGroup('bot'); setConnectionResult(null); setError(null); }}
              disabled={isNetworkConnecting}
            >
              {t('remoteConnect.groupBot')}
              {isBotConnected && <span className="bitfun-remote-connect__dot" />}
            </button>
          </div>

          {/* ── Sub-tabs ── */}
          {activeGroup === 'network' ? (
            <div className="bitfun-remote-connect__subtabs">
              {NETWORK_TABS.map((tab, i) => (
                <React.Fragment key={tab.id}>
                  {i > 0 && <span className="bitfun-remote-connect__subtab-divider" />}
                  <button
                    type="button"
                    className={`bitfun-remote-connect__subtab${networkTab === tab.id ? ' is-active' : ''}${isRelayConnected && connectedNetworkTab === tab.id ? ' is-connected' : ''}`}
                    onClick={() => { setNetworkTab(tab.id); setConnectionResult(null); setError(null); }}
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
            <div className="bitfun-remote-connect__subtabs">
              {BOT_TABS.map((tab, i) => (
                <React.Fragment key={tab.id}>
                  {i > 0 && <span className="bitfun-remote-connect__subtab-divider" />}
                  <button
                    type="button"
                    className={`bitfun-remote-connect__subtab${botTab === tab.id ? ' is-active' : ''}${isBotConnected && connectedBotTab === tab.id ? ' is-connected' : ''}`}
                    onClick={() => { setBotTab(tab.id); setConnectionResult(null); setError(null); }}
                    disabled={isBotSubDisabled(tab.id) || isBotConnecting}
                  >
                    {tab.id === 'feishu' ? t('remoteConnect.feishu') : tab.label}
                    {isBotConnected && connectedBotTab === tab.id && botTab !== tab.id && (
                      <span className="bitfun-remote-connect__dot-sm" />
                    )}
                  </button>
                </React.Fragment>
              ))}
            </div>
          )}

          {/* ── Content ── */}
          {activeGroup === 'network' ? renderNetworkContent() : renderBotContent()}
        </div>
      </Modal>

      <Modal
        isOpen={showDisclaimer}
        onClose={() => setShowDisclaimer(false)}
        title={t('remoteConnect.disclaimerTitle')}
        showCloseButton
        size="large"
        contentInset
      >
        <RemoteConnectDisclaimerContent
          agreed={hasAgreedDisclaimer}
          onClose={() => setShowDisclaimer(false)}
          onAgree={hasAgreedDisclaimer ? undefined : handleAgreeDisclaimer}
        />
      </Modal>
    </>
  );
};

export default RemoteConnectDialog;
