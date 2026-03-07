import React from 'react';
import { Play, Trash2 } from 'lucide-react';
import * as LucideIcons from 'lucide-react';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { Tag } from '@/component-library';
import './MiniAppCard.scss';

interface MiniAppCardProps {
  app: MiniAppMeta;
  index?: number;
  isRunning?: boolean;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
}

function getIcon(name: string): React.ReactNode {
  const key = name
    .split('-')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('') as keyof typeof LucideIcons;
  const Icon = LucideIcons[key] as React.ElementType | undefined;
  return Icon ? <Icon size={28} strokeWidth={1.5} /> : <LucideIcons.Box size={28} strokeWidth={1.5} />;
}

function getIconGradient(icon: string): string {
  const gradients = [
    'linear-gradient(135deg, rgba(59,130,246,0.35) 0%, rgba(139,92,246,0.25) 100%)',
    'linear-gradient(135deg, rgba(16,185,129,0.3) 0%, rgba(59,130,246,0.25) 100%)',
    'linear-gradient(135deg, rgba(245,158,11,0.3) 0%, rgba(239,68,68,0.2) 100%)',
    'linear-gradient(135deg, rgba(139,92,246,0.35) 0%, rgba(236,72,153,0.2) 100%)',
    'linear-gradient(135deg, rgba(6,182,212,0.3) 0%, rgba(59,130,246,0.25) 100%)',
    'linear-gradient(135deg, rgba(239,68,68,0.25) 0%, rgba(245,158,11,0.2) 100%)',
  ];
  const idx = (icon.charCodeAt(0) || 0) % gradients.length;
  return gradients[idx];
}

const MiniAppCard: React.FC<MiniAppCardProps> = ({
  app,
  index = 0,
  isRunning = false,
  onOpen,
  onDelete,
}) => {
  const handleDeleteClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete(app.id);
  };

  const handleOpenClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onOpen(app.id);
  };

  return (
    <div
      className={['miniapp-card', isRunning && 'miniapp-card--running'].filter(Boolean).join(' ')}
      style={{ '--card-index': index } as React.CSSProperties}
      onClick={() => onOpen(app.id)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && onOpen(app.id)}
      aria-label={`Open ${app.name}`}
    >
      <div
        className="miniapp-card__icon-area"
        style={{ background: getIconGradient(app.icon || 'box') }}
      >
        <div className="miniapp-card__icon">{getIcon(app.icon || 'box')}</div>
        {isRunning && <span className="miniapp-card__running-badge">运行中</span>}
      </div>

      <div className="miniapp-card__overlay">
        <button
          className="miniapp-card__overlay-btn miniapp-card__overlay-btn--primary"
          onClick={handleOpenClick}
          aria-label="Open"
        >
          <Play size={16} fill="currentColor" strokeWidth={0} />
        </button>
        <button
          className="miniapp-card__overlay-btn miniapp-card__overlay-btn--danger"
          onClick={handleDeleteClick}
          aria-label="Delete"
        >
          <Trash2 size={14} />
        </button>
      </div>

      <div className="miniapp-card__info">
        <div className="miniapp-card__name">{app.name}</div>
        {app.description && <div className="miniapp-card__desc">{app.description}</div>}
        {app.tags.length > 0 && (
          <div className="miniapp-card__tags">
            {app.tags.slice(0, 2).map((tag) => (
              <Tag key={tag} color="gray" size="small" rounded>
                {tag}
              </Tag>
            ))}
          </div>
        )}
      </div>

      <div className="miniapp-card__version">v{app.version}</div>
    </div>
  );
};

export default MiniAppCard;

