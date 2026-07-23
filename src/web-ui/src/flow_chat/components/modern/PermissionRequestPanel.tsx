import { useState, type CSSProperties } from 'react';
import { Check, ChevronsDown, ShieldAlert, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import type {
  PermissionReplyKind,
  PermissionRequest,
} from '@/infrastructure/api/service-api/AgentAPI';
import { useChatInputState } from '../../store/chatInputStateStore';
import { CHAT_INPUT_DROP_ZONE_BOTTOM_PX } from '../../utils/flowChatScrollLayout';
import './PermissionRequestPanel.scss';

const PERMISSION_PANEL_INPUT_GAP_PX = 16;

interface PermissionRequestPanelProps {
  requests: PermissionRequest[];
  onRespond: (requestId: string, reply: PermissionReplyKind, feedback?: string) => Promise<void>;
  onRespondBatch: (requestId: string, reply: PermissionReplyKind, feedback?: string) => Promise<void>;
  aboveChatInput?: boolean;
  totalPendingCount?: number;
}

const PERMISSION_ACTION_LABEL_KEYS: Record<string, string> = {
  read: 'permission.actions.read',
  edit: 'permission.actions.edit',
  bash: 'permission.actions.bash',
  git: 'permission.actions.git',
  computer_use: 'permission.actions.computerUse',
  websearch: 'permission.actions.webSearch',
  webfetch: 'permission.actions.webFetch',
  mcp: 'permission.actions.mcp',
  task: 'permission.actions.task',
  skill: 'permission.actions.skill',
  page_publish: 'permission.actions.pagePublish',
  page_deploy: 'permission.actions.pageDeploy',
  custom_tool: 'permission.actions.customTool',
  external_directory: 'permission.actions.externalDirectory',
};

const PAGE_VISIBILITY_LABEL_KEYS: Record<string, string> = {
  private: 'permission.visibility.private',
  relay: 'permission.visibility.relay',
  public: 'permission.visibility.public',
};

function permissionActionLabel(action: string, t: (key: string) => string): string {
  return t(PERMISSION_ACTION_LABEL_KEYS[action] ?? 'permission.actions.other');
}

function metadataString(metadata: Record<string, unknown> | undefined, key: string): string | undefined {
  const value = metadata?.[key];
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

function permissionRisk(
  request: PermissionRequest | undefined,
  t: (key: string, values?: Record<string, string>) => string,
): string | undefined {
  if (!request) return undefined;
  const metadata = request.displayMetadata;
  const operation = metadataString(metadata, 'pageOperation');
  const slug = metadataString(metadata, 'pageSlug');
  if (operation && slug) {
    if (operation === 'deploy') {
      return t('permission.risks.pageDeploy', {
        slug,
        version: metadataString(metadata, 'pageVersion') ?? '',
      });
    }
    const visibility = metadataString(metadata, 'pageVisibility') ?? 'private';
    const translatedVisibility = t(
      PAGE_VISIBILITY_LABEL_KEYS[visibility] ?? PAGE_VISIBILITY_LABEL_KEYS.private,
    );
    return t(
      operation === 'publish' ? 'permission.risks.pagePublish' : 'permission.risks.pageSave',
      { slug, visibility: translatedVisibility },
    );
  }
  return [metadata?.riskDescription, metadata?.risk].find(
    (value): value is string => typeof value === 'string' && value.trim().length > 0,
  );
}

export function PermissionRequestPanel({
  requests,
  onRespond,
  onRespondBatch,
  aboveChatInput = false,
  totalPendingCount,
}: PermissionRequestPanelProps) {
  const { t } = useTranslation('flow-chat');
  const [feedback, setFeedback] = useState('');
  const [responding, setResponding] = useState(false);
  const [error, setError] = useState(false);
  const [isCollapsed, setIsCollapsed] = useState(false);
  const inputHeight = useChatInputState((state) => state.inputHeight);
  const request = requests[0];
  const risk = permissionRisk(request, t);
  const pendingCount = Math.max(totalPendingCount ?? requests.length, requests.length);

  const alwaysAllowTooltip = request?.saveResources?.length
    ? request.projectPath?.trim()
      ? t('permission.allowAlwaysTooltip', { projectPath: request.projectPath.trim() })
      : t('permission.allowAlwaysTooltipCurrentProject')
    : t('permission.allowAlwaysTooltipNoGrant');

  const panelStyle = aboveChatInput && inputHeight > 0
    ? {
        '--permission-request-panel-bottom': `${
          inputHeight + CHAT_INPUT_DROP_ZONE_BOTTOM_PX + PERMISSION_PANEL_INPUT_GAP_PX
        }px`,
      } as CSSProperties
    : undefined;

  const respond = async (reply: PermissionReplyKind) => {
    setResponding(true);
    setError(false);
    try {
      await onRespond(request.requestId, reply, reply === 'reject' ? feedback : undefined);
    } catch {
      setError(true);
    } finally {
      setResponding(false);
    }
  };

  const respondBatch = async (reply: PermissionReplyKind) => {
    setResponding(true);
    setError(false);
    try {
      await onRespondBatch(request.requestId, reply, reply === 'reject' ? feedback : undefined);
    } catch {
      setError(true);
    } finally {
      setResponding(false);
    }
  };

  if (!request) return null;

  return (
    <div
      className={`permission-request-anchor${aboveChatInput ? ' permission-request-anchor--above-chat-input' : ''}`}
      style={panelStyle}
    >
      {isCollapsed ? (
        <Tooltip content={t('permission.expandPanel', { count: pendingCount })} placement="top">
          <button
            type="button"
            className="permission-request-panel__collapsed-trigger"
            onClick={() => setIsCollapsed(false)}
            aria-label={t('permission.expandPanel', { count: pendingCount })}
            aria-expanded={false}
            data-testid="permission-request-panel-expand"
          >
            <ShieldAlert size={21} aria-hidden="true" />
            <span className="permission-request-panel__collapsed-badge" aria-hidden="true">
              {pendingCount > 99 ? '99+' : pendingCount}
            </span>
          </button>
        </Tooltip>
      ) : (
        <section
          id="permission-request-panel"
          className="permission-request-panel"
          aria-label={t('permission.title')}
        >
          <div className="permission-request-panel__heading">
            <div className="permission-request-panel__heading-title">
              <ShieldAlert size={18} aria-hidden="true" />
              <h2>{t('permission.title')}</h2>
            </div>
            <div className="permission-request-panel__heading-actions">
              <span className="permission-request-panel__count">
                {t('permission.batchCount', { count: requests.length })}
              </span>
              <Tooltip content={t('permission.collapsePanel')} placement="top">
                <button
                  type="button"
                  className="permission-request-panel__collapse"
                  onClick={() => setIsCollapsed(true)}
                  aria-label={t('permission.collapsePanel')}
                  aria-expanded={true}
                  data-testid="permission-request-panel-collapse"
                >
                  <ChevronsDown size={17} aria-hidden="true" />
                </button>
              </Tooltip>
            </div>
          </div>
          <div className="permission-request-panel__requests" role="list">
            {requests.map((item, index) => (
              <div
                className={`permission-request-panel__request${index === 0 ? ' permission-request-panel__request--active' : ''}`}
                key={item.requestId}
                role="listitem"
              >
                <div className="permission-request-panel__request-heading">
                  <div className="permission-request-panel__tool-identity">
                    <strong>{item.source.identity}</strong>
                    {item.delegation && (
                      <span className="permission-request-panel__subagent">
                        {t('permission.subagentOwner', { subagent: item.delegation.subagentType })}
                      </span>
                    )}
                  </div>
                  <span>{index === 0 ? t('permission.current') : t('permission.pending')}</span>
                </div>
                <div className="permission-request-panel__request-details">
                  <span className="permission-request-panel__action">
                    {permissionActionLabel(item.action, t)}
                  </span>
                  <span className="permission-request-panel__detail-separator" aria-hidden="true">·</span>
                  <Tooltip content={item.resources.join(', ')} placement="top">
                    <code className="permission-request-panel__resource-summary">
                      {item.resources.join(', ')}
                    </code>
                  </Tooltip>
                </div>
              </div>
            ))}
          </div>
          {risk && <p className="permission-request-panel__risk">{risk}</p>}
          {error && <p role="alert">{t('permission.responseFailed')}</p>}
          <textarea
            value={feedback}
            onChange={(event) => setFeedback(event.target.value)}
            placeholder={t('permission.feedbackPlaceholder')}
            aria-label={t('permission.feedbackLabel')}
            disabled={responding}
            rows={2}
          />
          <div className="permission-request-panel__actions">
            <div className="permission-request-panel__single-actions">
              <button type="button" onClick={() => void respond('once')} disabled={responding}>
                <Check size={15} aria-hidden="true" /> {t('permission.allowOnce')}
              </button>
              {!!request.saveResources?.length && (
                <Tooltip content={alwaysAllowTooltip} placement="top">
                  <button type="button" onClick={() => void respond('always')} disabled={responding}>
                    <Check size={15} aria-hidden="true" /> {t('permission.allowAlways')}
                  </button>
                </Tooltip>
              )}
              <button
                type="button"
                className="permission-request-panel__reject"
                onClick={() => void respond('reject')}
                disabled={responding}
              >
                <X size={15} aria-hidden="true" /> {t('permission.reject')}
              </button>
            </div>
            {requests.length > 1 && (
              <div className="permission-request-panel__batch-actions">
                <button type="button" onClick={() => void respondBatch('once')} disabled={responding}>
                  <Check size={15} aria-hidden="true" /> {t('permission.allowCurrentAndFollowing')}
                </button>
                <button
                  type="button"
                  className="permission-request-panel__reject"
                  onClick={() => void respondBatch('reject')}
                  disabled={responding}
                >
                  <X size={15} aria-hidden="true" /> {t('permission.rejectCurrentAndFollowing')}
                </button>
              </div>
            )}
          </div>
        </section>
      )}
    </div>
  );
}
