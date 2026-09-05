import React, { useState } from 'react';
import { MobileButton, MobileIconButton, MobileSheet } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import type { RemoteToolStatus } from '../services/RemoteSessionManager';

/** Show only the payload actually supplied by the remote host. */
export default function ChatToolDetails({ tool, label }: { tool: RemoteToolStatus; label?: React.ReactNode }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const request = tool.tool_input == null ? tool.input_preview : tool.tool_input;
  const text = typeof request === 'string' ? request : request == null ? '' : JSON.stringify(request, null, 2);
  return (
    <>
      <MobileButton appearance="plain" className={label ? 'chat-tool-details__trigger chat-tool-details__trigger--inline' : 'chat-tool-details__trigger'} aria-label={`${t('chat.toolDetails')} · ${tool.name}`} onClick={() => setOpen(true)}>
        {label || t('chat.toolDetails')}
      </MobileButton>
      <MobileSheet
        open={open}
        onOpenChange={setOpen}
        title={tool.name}
        className="chat-tool-details"
        headerAction={<MobileIconButton appearance="plain" aria-label={t('common.close')} onClick={() => setOpen(false)} icon={<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg>} />}
      >
        <h3>{t('chat.toolRequest')}</h3>
        {tool.tool_input == null && <p>{t(text ? 'chat.requestPreview' : 'chat.requestUnavailable')}</p>}
        {text && <pre className="chat-tool-details__payload">{text}</pre>}
      </MobileSheet>
    </>
  );
}
