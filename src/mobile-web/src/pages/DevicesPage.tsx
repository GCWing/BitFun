/**
 * Devices Page — list all same-account devices, browse their sessions,
 * and send messages to remote sessions.
 *
 * Requires a delegated identity (token + master_key) from the paired
 * desktop's account login.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { useI18n } from '../i18n';

interface DeviceInfo {
  device_id: string;
  device_name: string;
  online: boolean;
}

interface RemoteSession {
  session_id: string;
  name: string;
  agent_type: string;
  message_count: number;
}

interface RemoteMessage {
  id: string;
  role: string;
  content: string;
}

type View = 'devices' | 'sessions' | 'chat';

interface Props {
  client: RelayHttpClient;
  onBack: () => void;
}

const DevicesPage: React.FC<Props> = ({ client, onBack }) => {
  const { t } = useI18n();
  const [view, setView] = useState<View>('devices');
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<DeviceInfo | null>(null);
  const [sessions, setSessions] = useState<RemoteSession[]>([]);
  const [messages, setMessages] = useState<RemoteMessage[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [messageInput, setMessageInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshDevices = useCallback(async () => {
    if (!client.hasDelegatedIdentity) return;
    try {
      const list = await client.listDevices();
      setDevices(list);
    } catch (e) {
      // silent
    }
  }, [client]);

  useEffect(() => {
    refreshDevices();
    const interval = setInterval(refreshDevices, 10000);
    return () => clearInterval(interval);
  }, [refreshDevices]);

  const selectDevice = useCallback(async (d: DeviceInfo) => {
    if (!d.online) return;
    setSelectedDevice(d);
    setLoading(true); setError(null);
    try {
      const wsInfo = await client.sendDeviceRpc<any>(d.device_id, { cmd: 'get_workspace_info' });
      const sessResp = await client.sendDeviceRpc<any>(d.device_id, {
        cmd: 'list_sessions', workspace_path: wsInfo.path || '/', limit: 50,
      });
      setSessions(sessResp.sessions || []);
      setView('sessions');
    } catch (e: any) {
      setError(e.message || 'Failed to load device');
    } finally { setLoading(false); }
  }, [client]);

  const selectSession = useCallback(async (sessionId: string) => {
    if (!selectedDevice) return;
    setSelectedSession(sessionId);
    setLoading(true); setError(null);
    try {
      const data = await client.sendDeviceRpc<any>(selectedDevice.device_id, {
        cmd: 'get_session_messages', session_id: sessionId, limit: 100,
      });
      setMessages(data.messages || []);
      setView('chat');
    } catch (e: any) {
      setError(e.message || 'Failed to load messages');
    } finally { setLoading(false); }
  }, [client, selectedDevice]);

  const sendMessage = useCallback(async () => {
    if (!selectedDevice || !selectedSession || !messageInput.trim()) return;
    setLoading(true); setError(null);
    try {
      await client.sendDeviceRpc(selectedDevice.device_id, {
        cmd: 'send_message', session_id: selectedSession,
        content: messageInput, agent_type: null, images: null, image_contexts: null,
      });
      setMessageInput('');
      const data = await client.sendDeviceRpc<any>(selectedDevice.device_id, {
        cmd: 'get_session_messages', session_id: selectedSession, limit: 100,
      });
      setMessages(data.messages || []);
    } catch (e: any) {
      setError(e.message || 'Failed to send message');
    } finally { setLoading(false); }
  }, [client, selectedDevice, selectedSession, messageInput]);

  const createSession = useCallback(async () => {
    if (!selectedDevice) return;
    setLoading(true); setError(null);
    try {
      const data = await client.sendDeviceRpc<any>(selectedDevice.device_id, {
        cmd: 'create_session', agent_type: null, session_name: null, workspace_path: '/',
      });
      if (data.session_id) {
        await selectDevice(selectedDevice);
        await selectSession(data.session_id);
      }
    } catch (e: any) {
      setError(e.message || 'Failed to create session');
    } finally { setLoading(false); }
  }, [client, selectedDevice, selectDevice, selectSession]);

  if (!client.hasDelegatedIdentity) {
    return (
      <div className="devices-page">
        <div className="devices-page__header">
          <button className="devices-page__back" onClick={onBack}>←</button>
          <h2>{t('accountLogin.devices') || 'Devices'}</h2>
        </div>
        <div className="devices-page__empty">
          The paired desktop is not logged into a BitFun account.
          Please log in on the desktop to enable multi-device control.
        </div>
      </div>
    );
  }

  return (
    <div className="devices-page">
      <div className="devices-page__header">
        {view !== 'devices' && (
          <button className="devices-page__back" onClick={() => {
            if (view === 'chat') setView('sessions');
            else setView('devices');
          }}>←</button>
        )}
        <h2>{view === 'devices' ? (t('accountLogin.devices') || 'Devices')
          : view === 'sessions' ? (selectedDevice?.device_name || 'Sessions')
          : 'Chat'}</h2>
      </div>

      {error && <div className="devices-page__error">{error}</div>}

      {loading && <div className="devices-page__loading">Loading...</div>}

      {/* Device list */}
      {view === 'devices' && !loading && (
        <div className="devices-page__list">
          {devices.length === 0 && <div className="devices-page__empty">No devices found</div>}
          {devices.map(d => (
            <div key={d.device_id}
              className={`devices-page__device ${d.online ? '' : 'offline'}`}
              onClick={() => selectDevice(d)}>
              <span className="devices-page__device-name">{d.device_name}</span>
              <span className="devices-page__device-status">
                {d.online ? '🟢' : '⚪'} {d.device_id.slice(0, 8)}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Session list */}
      {view === 'sessions' && selectedDevice && !loading && (
        <div className="devices-page__list">
          <button className="devices-page__new-btn" onClick={createSession}>
            + New Session
          </button>
          {sessions.length === 0 && <div className="devices-page__empty">No sessions</div>}
          {sessions.map(s => (
            <div key={s.session_id} className="devices-page__session"
              onClick={() => selectSession(s.session_id)}>
              <span className="devices-page__session-name">{s.name}</span>
              <span className="devices-page__session-meta">{s.agent_type} · {s.message_count}</span>
            </div>
          ))}
        </div>
      )}

      {/* Chat */}
      {view === 'chat' && selectedDevice && selectedSession && (
        <>
          <div className="devices-page__messages">
            {messages.map((msg, i) => (
              <div key={i} className={`devices-page__msg devices-page__msg--${msg.role}`}>
                <div className="devices-page__msg-role">{msg.role}</div>
                <div className="devices-page__msg-content">{msg.content}</div>
              </div>
            ))}
          </div>
          <div className="devices-page__input-bar">
            <input type="text" value={messageInput}
              onChange={e => setMessageInput(e.target.value)}
              placeholder="Send message..."
              onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); sendMessage(); } }}
              disabled={loading} />
            <button onClick={sendMessage} disabled={loading || !messageInput.trim()}>Send</button>
          </div>
        </>
      )}
    </div>
  );
};

export default DevicesPage;
