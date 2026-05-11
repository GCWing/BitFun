import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Zap, GitCommitHorizontal, GitPullRequest, Pencil, Trash2, Plus, Check } from 'lucide-react';
import {
  Button,
  ConfigPageLoading,
  IconButton,
  Modal,
  Switch,
  Input,
  Textarea,
} from '@/component-library';
import {
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageContent,
  ConfigPageSection,
} from './common';
import {
  aiExperienceConfigService,
  DEFAULT_QUICK_ACTIONS,
  type QuickAction,
} from '../services/AIExperienceConfigService';
import {
  normalizeQuickActionTextForStorage,
  resolveQuickActionText,
} from '../services/quickActionLocalization';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './QuickActionsConfig.scss';

const log = createLogger('QuickActionsConfig');

const BUILTIN_IDS = new Set(['commit', 'create_pr']);

type TranslationFn = (key: string, options?: Record<string, unknown>) => string;

function getActionIcon(id: string, size = 15) {
  if (id === 'commit') return <GitCommitHorizontal size={size} />;
  if (id === 'create_pr') return <GitPullRequest size={size} />;
  return <Zap size={size} />;
}

// ── ActionFormModal ─────────────────────────────────────────────────────────

interface ActionFormModalProps {
  isOpen: boolean;
  /** undefined = create mode, QuickAction = edit mode */
  target: QuickAction | undefined;
  onClose: () => void;
  onSubmit: (label: string, prompt: string) => void;
  t: TranslationFn;
}

const ActionFormModal: React.FC<ActionFormModalProps> = ({ isOpen, target, onClose, onSubmit, t }) => {
  const [label, setLabel] = useState('');
  const [prompt, setPrompt] = useState('');
  const labelInputRef = useRef<HTMLInputElement>(null);

  // Sync form when target changes or modal opens.
  useEffect(() => {
    if (isOpen) {
      const targetText = target ? resolveQuickActionText(target, t) : undefined;
      setLabel(targetText?.label ?? '');
      setPrompt(targetText?.prompt ?? '');
      // Delay focus so the modal animation completes first.
      setTimeout(() => labelInputRef.current?.focus(), 80);
    }
  }, [isOpen, t, target]);

  const canSubmit = label.trim().length > 0 && prompt.trim().length > 0;

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') { onClose(); return; }
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && canSubmit) {
      onSubmit(label.trim(), prompt.trim());
    }
  };

  const isEdit = !!target;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={isEdit ? t('modal.editTitle') : t('modal.addTitle')}
      size="medium"
      contentInset
    >
      <div className="quick-actions-config__modal-body" onKeyDown={handleKeyDown}>
        {target && (
          <div className="quick-actions-config__modal-icon-preview">
            <div className="quick-actions-config__modal-action-icon">
              {getActionIcon(target.id, 18)}
            </div>
          </div>
        )}

        <div className="quick-actions-config__modal-field">
          <label className="quick-actions-config__modal-label" htmlFor="qa-label">
            {t('modal.labelField')}
          </label>
          <Input
            ref={labelInputRef}
            id="qa-label"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder={t('modal.labelPlaceholder')}
          />
        </div>

        <div className="quick-actions-config__modal-field">
          <label className="quick-actions-config__modal-label" htmlFor="qa-prompt">
            {t('modal.promptField')}
          </label>
          <Textarea
            id="qa-prompt"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder={t('modal.promptPlaceholder')}
            rows={4}
            autoResize
            className="quick-actions-config__modal-textarea"
          />
          <p className="quick-actions-config__modal-hint">{t('modal.promptHint')}</p>
        </div>

        <div className="quick-actions-config__modal-footer">
          <Button variant="ghost" size="small" onClick={onClose}>
            {t('modal.cancel')}
          </Button>
          <Button
            variant="primary"
            size="small"
            onClick={() => onSubmit(label.trim(), prompt.trim())}
            disabled={!canSubmit}
          >
            <Check size={14} />
            {isEdit ? t('modal.saveEdit') : t('modal.confirmAdd')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

// ── ActionRow ───────────────────────────────────────────────────────────────

interface ActionRowProps {
  action: QuickAction;
  onToggle: (id: string) => void;
  onEdit: (action: QuickAction) => void;
  onDelete: (id: string) => void;
  canDelete: boolean;
  t: TranslationFn;
}

const ActionRow: React.FC<ActionRowProps> = ({ action, onToggle, onEdit, onDelete, canDelete, t }) => {
  const actionText = resolveQuickActionText(action, t);

  return (
    <div className="quick-actions-config__row">
      <div className="quick-actions-config__row-icon">
        {getActionIcon(action.id)}
      </div>

      <div className="quick-actions-config__row-body">
        <div className="quick-actions-config__row-label">{actionText.label}</div>
        <div className="quick-actions-config__row-prompt">{actionText.prompt}</div>
      </div>

      <div className="quick-actions-config__row-controls">
        <Switch
          checked={action.enabled}
          onChange={() => onToggle(action.id)}
          size="small"
        />
        <IconButton
          type="button"
          size="small"
          variant="ghost"
          aria-label={t('edit.button')}
          tooltip={t('edit.button')}
          onClick={() => onEdit(action)}
        >
          <Pencil size={13} />
        </IconButton>
        {canDelete && (
          <IconButton
            type="button"
            size="small"
            variant="ghost"
            aria-label={t('delete.button')}
            tooltip={t('delete.button')}
            onClick={() => onDelete(action.id)}
            className="quick-actions-config__delete-btn"
          >
            <Trash2 size={13} />
          </IconButton>
        )}
      </div>
    </div>
  );
};

// ── Main page ───────────────────────────────────────────────────────────────

const QuickActionsConfig: React.FC = () => {
  const { t } = useTranslation('settings/quick-actions');
  const notification = useNotification();

  const [loading, setLoading] = useState(true);
  const [actions, setActions] = useState<QuickAction[]>([]);

  // Modal state: undefined = closed, null = create, QuickAction = edit
  const [modalTarget, setModalTarget] = useState<QuickAction | null | undefined>(undefined);
  const isModalOpen = modalTarget !== undefined;

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const settings = await aiExperienceConfigService.getSettingsAsync();
      const stored = settings.quick_actions;
      setActions((stored && stored.length > 0) ? stored : DEFAULT_QUICK_ACTIONS);
    } catch (error) {
      log.error('Failed to load quick actions', error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const persist = useCallback(async (next: QuickAction[]) => {
    try {
      const settings = await aiExperienceConfigService.getSettingsAsync();
      await aiExperienceConfigService.saveSettings({ ...settings, quick_actions: next });
      setActions(next);
      notification.success(t('messages.saved'));
    } catch (error) {
      log.error('Failed to save quick actions', error);
      notification.error(t('messages.saveFailed'));
    }
  }, [notification, t]);

  const handleToggle = useCallback((id: string) => {
    void persist(actions.map(a => a.id === id ? { ...a, enabled: !a.enabled } : a));
  }, [actions, persist]);

  const handleDelete = useCallback((id: string) => {
    void persist(actions.filter(a => a.id !== id));
  }, [actions, persist]);

  const handleModalSubmit = useCallback((label: string, prompt: string) => {
    if (modalTarget === null) {
      // Create mode
      const newAction: QuickAction = {
        id: `custom_${Date.now()}`,
        label,
        prompt,
        enabled: true,
      };
      void persist([...actions, newAction]);
    } else if (modalTarget) {
      // Edit mode
      const normalizedText = normalizeQuickActionTextForStorage(modalTarget, label, prompt, t);
      void persist(actions.map(a => a.id === modalTarget.id ? { ...a, ...normalizedText } : a));
    }
    setModalTarget(undefined);
  }, [actions, modalTarget, persist, t]);

  if (loading) {
    return (
      <ConfigPageLayout className="quick-actions-config">
        <ConfigPageHeader title={t('page.title')} subtitle={t('page.subtitle')} />
        <ConfigPageContent>
          <ConfigPageLoading text={t('loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  const builtinActions = actions.filter(a => BUILTIN_IDS.has(a.id));
  const customActions = actions.filter(a => !BUILTIN_IDS.has(a.id));

  return (
    <ConfigPageLayout className="quick-actions-config">
      <ConfigPageHeader title={t('page.title')} subtitle={t('page.subtitle')} />

      <ConfigPageContent className="quick-actions-config__content">

        {/* ── Built-in actions ──────────────────────────────────────────── */}
        <ConfigPageSection
          title={t('sections.builtin.title')}
          description={t('sections.builtin.description')}
        >
          <div className="quick-actions-config__list">
            {builtinActions.map(action => (
              <ActionRow
                key={action.id}
                action={action}
                onToggle={handleToggle}
                onEdit={(a) => setModalTarget(a)}
                onDelete={handleDelete}
                canDelete={false}
                t={t}
              />
            ))}
          </div>
        </ConfigPageSection>

        {/* ── Custom actions ────────────────────────────────────────────── */}
        <ConfigPageSection
          title={t('sections.custom.title')}
          description={t('sections.custom.description')}
          extra={
            <Button
              size="small"
              variant="secondary"
              onClick={() => setModalTarget(null)}
            >
              <Plus size={14} />
              {t('add.button')}
            </Button>
          }
        >
          <div className="quick-actions-config__list">
            {customActions.length === 0 ? (
              <div className="quick-actions-config__empty">
                <Zap size={20} className="quick-actions-config__empty-icon" />
                <p>{t('sections.custom.empty')}</p>
                <Button
                  size="small"
                  variant="secondary"
                  onClick={() => setModalTarget(null)}
                >
                  <Plus size={14} />
                  {t('add.button')}
                </Button>
              </div>
            ) : (
              customActions.map(action => (
                <ActionRow
                  key={action.id}
                  action={action}
                  onToggle={handleToggle}
                  onEdit={(a) => setModalTarget(a)}
                  onDelete={handleDelete}
                  canDelete
                  t={t}
                />
              ))
            )}
          </div>
        </ConfigPageSection>

      </ConfigPageContent>

      <ActionFormModal
        isOpen={isModalOpen}
        target={modalTarget ?? undefined}
        onClose={() => setModalTarget(undefined)}
        onSubmit={handleModalSubmit}
        t={t}
      />
    </ConfigPageLayout>
  );
};

export default QuickActionsConfig;
