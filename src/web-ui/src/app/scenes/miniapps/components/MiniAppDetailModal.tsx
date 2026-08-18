import React, { useMemo, useRef } from 'react';
import {
  AppWindow,
  Bot,
  CheckCircle2,
  Cpu,
  Database,
  Download,
  FolderKanban,
  Globe2,
  Play,
  ShieldCheck,
  Sparkles,
  Square,
  Terminal,
  Trash2,
} from 'lucide-react';
import { Button, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { renderMiniAppIcon } from '../utils/miniAppIcons';
import { pickLocalizedString, pickLocalizedTags } from '../utils/pickLocalizedString';
import {
  projectMiniAppDetailCapabilities,
  resolveMiniAppDetailSource,
  type MiniAppDetailCapability,
  type MiniAppDetailSource,
} from './miniAppDetailPresentation';
import './MiniAppDetailModal.scss';

interface MiniAppDetailModalProps {
  app: MiniAppMeta | null;
  marketReleaseNumber?: number;
  isActive: boolean;
  isCustomizing: boolean;
  onClose: () => void;
  onOpen: (appId: string) => void;
  onDelete: (appId: string) => void;
  onStop?: (appId: string) => void | Promise<void>;
}

interface DetailSnapshot {
  app: MiniAppMeta;
  marketReleaseNumber?: number;
  isActive: boolean;
  isCustomizing: boolean;
}

type Translate = (key: string, params?: Record<string, unknown>) => string;

function capabilityIcon(capability: MiniAppDetailCapability): React.ReactNode {
  switch (capability.kind) {
    case 'ai': return <Sparkles size={28} strokeWidth={1.6} />;
    case 'workspace': return <FolderKanban size={28} strokeWidth={1.6} />;
    case 'export': return <Download size={28} strokeWidth={1.6} />;
    case 'shell': return <Terminal size={28} strokeWidth={1.6} />;
    case 'network': return <Globe2 size={28} strokeWidth={1.6} />;
    case 'worker': return <Cpu size={28} strokeWidth={1.6} />;
    case 'storage': return <Database size={28} strokeWidth={1.6} />;
    case 'desktop': return <Bot size={28} strokeWidth={1.6} />;
    case 'controlled': return <ShieldCheck size={28} strokeWidth={1.6} />;
    case 'instant': return <Play size={28} strokeWidth={1.6} />;
    case 'surface':
    default:
      return <AppWindow size={28} strokeWidth={1.6} />;
  }
}

function capabilityCopy(
  capability: MiniAppDetailCapability,
  t: Translate,
): { title: string; description: string } {
  switch (capability.kind) {
    case 'ai':
      return {
        title: t('detail.capabilities.ai.title'),
        description: t('detail.capabilities.ai.description'),
      };
    case 'workspace':
      return {
        title: t('detail.capabilities.workspace.title'),
        description: t('detail.capabilities.workspace.description'),
      };
    case 'export':
      return {
        title: t('detail.capabilities.export.title'),
        description: t('detail.capabilities.export.description'),
      };
    case 'shell':
      return {
        title: t('detail.capabilities.shell.title'),
        description: t('detail.capabilities.shell.description', { commands: capability.value ?? '' }),
      };
    case 'network':
      return {
        title: t('detail.capabilities.network.title'),
        description: t('detail.capabilities.network.description', { count: capability.count ?? 0 }),
      };
    case 'worker':
      return {
        title: t('detail.capabilities.worker.title'),
        description: t('detail.capabilities.worker.description'),
      };
    case 'storage':
      return {
        title: t('detail.capabilities.storage.title'),
        description: t('detail.capabilities.storage.description'),
      };
    case 'desktop':
      return {
        title: t('detail.capabilities.desktop.title'),
        description: t('detail.capabilities.desktop.description', { count: capability.count ?? 0 }),
      };
    case 'controlled':
      return {
        title: t('detail.capabilities.controlled.title'),
        description: t('detail.capabilities.controlled.description'),
      };
    case 'instant':
      return {
        title: t('detail.capabilities.instant.title'),
        description: t('detail.capabilities.instant.description'),
      };
    case 'surface':
    default:
      return {
        title: t('detail.capabilities.surface.title'),
        description: t('detail.capabilities.surface.description'),
      };
  }
}

function sourceLabel(source: MiniAppDetailSource, t: Translate): string {
  switch (source) {
    case 'builtin': return t('detail.source.builtin');
    case 'market': return t('detail.source.market');
    case 'installed':
    default:
      return t('detail.source.installed');
  }
}

const MiniAppDetailModal: React.FC<MiniAppDetailModalProps> = ({
  app,
  marketReleaseNumber,
  isActive,
  isCustomizing,
  onClose,
  onOpen,
  onDelete,
  onStop,
}) => {
  const { t, currentLanguage } = useI18n('scenes/miniapp');
  const snapshotRef = useRef<DetailSnapshot | null>(null);

  if (app) {
    snapshotRef.current = {
      app,
      marketReleaseNumber,
      isActive,
      isCustomizing,
    };
  }

  const snapshot = app
    ? { app, marketReleaseNumber, isActive, isCustomizing }
    : snapshotRef.current;

  const displayedApp = snapshot?.app;
  const capabilities = useMemo(
    () => displayedApp ? projectMiniAppDetailCapabilities(displayedApp) : [],
    [displayedApp],
  );

  if (!displayedApp || !snapshot) return null;

  const localizedName = pickLocalizedString(displayedApp, currentLanguage, 'name');
  const localizedDescription = pickLocalizedString(displayedApp, currentLanguage, 'description');
  const localizedTags = pickLocalizedTags(displayedApp, currentLanguage);
  const source = resolveMiniAppDetailSource(displayedApp.id, snapshot.marketReleaseNumber !== undefined);
  const version = snapshot.marketReleaseNumber ?? displayedApp.version;
  const statusCopy = snapshot.isCustomizing
    ? t('detail.status.customizing')
    : snapshot.isActive
      ? t('detail.status.running')
      : t('detail.status.installed');

  return (
    <Modal
      isOpen={Boolean(app)}
      onClose={onClose}
      size="xxlarge"
      title={localizedName}
      titleExtra={(
        <span className="miniapp-detail-modal__source-badge" data-testid="miniapp-detail-source">
          {sourceLabel(source, t)}
        </span>
      )}
      overlayClassName="miniapp-detail-overlay"
      contentClassName="miniapp-detail-modal__modal-content"
      testId="miniapp-detail-dialog"
      titleTestId="miniapp-detail-title"
      closeButtonTestId="miniapp-detail-close"
    >
      <article
        className="miniapp-detail-modal"
        data-bf-component="mini-app-detail-modal"
        data-bf-part="root"
        data-bf-state={[
          snapshot.isActive && 'running',
          snapshot.isCustomizing && 'customizing',
        ].filter(Boolean).join(' ') || undefined}
        data-bf-source={source}
        data-miniapp-id={displayedApp.id}
      >
        <section className="miniapp-detail-modal__hero" data-bf-component="mini-app-detail-modal" data-bf-part="hero">
          <div className="miniapp-detail-modal__icon-stage" data-bf-component="mini-app-detail-modal" data-bf-part="iconStage">
            <div className="miniapp-detail-modal__icon" data-bf-component="mini-app-detail-modal" data-bf-part="icon">
              {renderMiniAppIcon(displayedApp.icon || 'box', 72)}
            </div>
            {snapshot.isActive ? (
              <span
                className="miniapp-detail-modal__activity-dot"
                aria-label={t('detail.activity.running')}
                data-testid="miniapp-detail-activity"
              />
            ) : null}
          </div>

          <div className="miniapp-detail-modal__summary" data-bf-component="mini-app-detail-modal" data-bf-part="summary">
            <div className="miniapp-detail-modal__identity">
              <h3 className="miniapp-detail-modal__name">{localizedName}</h3>
              <span className="miniapp-detail-modal__version" data-testid="miniapp-detail-version">v{version}</span>
            </div>
            {localizedDescription.trim() ? (
              <p className="miniapp-detail-modal__description" data-testid="miniapp-detail-description">
                {localizedDescription.trim()}
              </p>
            ) : null}
            {localizedTags.length ? (
              <div className="miniapp-detail-modal__tags" data-bf-component="mini-app-detail-modal" data-bf-part="tags">
                {localizedTags.map((tag) => (
                  <span key={tag} className="miniapp-detail-modal__tag">{tag}</span>
                ))}
              </div>
            ) : null}
          </div>
        </section>

        <section className="miniapp-detail-modal__highlights" data-bf-component="mini-app-detail-modal" data-bf-part="highlights">
          <h4 className="miniapp-detail-modal__section-title">{t('detail.highlights')}</h4>
          <div className="miniapp-detail-modal__highlight-grid">
            {capabilities.map((capability) => {
              const copy = capabilityCopy(capability, t);
              return (
                <div
                  key={capability.kind}
                  className="miniapp-detail-modal__highlight"
                  data-bf-component="mini-app-detail-modal"
                  data-bf-part="highlight"
                  data-capability={capability.kind}
                  data-testid="miniapp-detail-capability"
                >
                  <span className="miniapp-detail-modal__highlight-icon" aria-hidden="true">
                    {capabilityIcon(capability)}
                  </span>
                  <span className="miniapp-detail-modal__highlight-copy">
                    <strong>{copy.title}</strong>
                    <span>{copy.description}</span>
                  </span>
                </div>
              );
            })}
          </div>
        </section>

        <footer className="miniapp-detail-modal__footer" data-bf-component="mini-app-detail-modal" data-bf-part="footer">
          <div className="miniapp-detail-modal__status" data-bf-component="mini-app-detail-modal" data-bf-part="status">
            <CheckCircle2 size={18} strokeWidth={1.8} aria-hidden="true" />
            <span data-testid="miniapp-detail-status">{statusCopy}</span>
          </div>
          <div className="miniapp-detail-modal__actions" data-bf-component="mini-app-detail-modal" data-bf-part="actions">
            {snapshot.isActive && onStop ? (
              <Button
                variant="secondary"
                size="medium"
                className="miniapp-detail-modal__stop"
                onClick={() => void onStop(displayedApp.id)}
                data-testid="miniapp-detail-stop"
              >
                <Square size={17} />
                {t('detail.stop')}
              </Button>
            ) : null}
            <Button
              variant="inverse"
              size="medium"
              className="miniapp-detail-modal__primary"
              onClick={() => onOpen(displayedApp.id)}
              data-testid="miniapp-detail-primary-action"
            >
              <Play size={18} fill="currentColor" strokeWidth={0} />
              {snapshot.isActive ? t('detail.open') : t('detail.start')}
            </Button>
            <Button
              variant="secondary"
              size="medium"
              className="miniapp-detail-modal__delete"
              onClick={() => onDelete(displayedApp.id)}
              data-testid="miniapp-detail-delete"
            >
              <Trash2 size={18} />
              {t('detail.delete')}
            </Button>
          </div>
        </footer>
      </article>
    </Modal>
  );
};

export default MiniAppDetailModal;
export type { MiniAppDetailModalProps };
