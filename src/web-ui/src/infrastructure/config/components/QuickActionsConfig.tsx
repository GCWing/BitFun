import {
  Button,
  Icon,
  IconButton,
  Input,
  Switch,
  Textarea,
  Tooltip,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@bitfun/ui';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Zap, GitCommitHorizontal, GitPullRequest } from 'lucide-react';
import { ConfigLoadingState } from '@/infrastructure/config/components/common';
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
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) onClose(); }}
      size="md"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{isEdit ? t('modal.editTitle') : t('modal.addTitle')}</DialogTitle>
        </DialogHeading>
        <DialogClose />
      </DialogHeader>
      <DialogBody>
      <div className="quick-actions-config__modal-body" onKeyDown={handleKeyDown} data-bf-component="quick-actions-config" data-bf-part="dialog">
        {target && (
          <div data-bf-component="quick-actions-config" data-bf-part="dialogIcon" className="quick-actions-config__modal-icon-preview">
            <div className="quick-actions-config__modal-action-icon">
              {getActionIcon(target.id, 18)}
            </div>
          </div>
        )}

        <div data-bf-component="quick-actions-config" data-bf-part="field" className="quick-actions-config__modal-field">
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

        <div data-bf-component="quick-actions-config" data-bf-part="field" className="quick-actions-config__modal-field">
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

        <div data-bf-component="quick-actions-config" data-bf-part="dialogFooter" className="quick-actions-config__modal-footer">
          <Button variant="outline" size="sm" onClick={onClose}>
            {t('modal.cancel')}
          </Button>
          <Button
            variant="fill"
            size="sm"
            onClick={() => onSubmit(label.trim(), prompt.trim())}
            disabled={!canSubmit}
            leadingIcon={<Icon name="check-line" size="sm" />}
          >

            {isEdit ? t('modal.saveEdit') : t('modal.confirmAdd')}
          </Button>
        </div>
      </div>
          </DialogBody>
    </Dialog>
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
    <div className="quick-actions-config__row" data-bf-component="quick-actions-config" data-bf-part="row">
      <div data-bf-component="quick-actions-config" data-bf-part="rowIcon" className="quick-actions-config__row-icon">
        {getActionIcon(action.id)}
      </div>

      <div data-bf-component="quick-actions-config" data-bf-part="rowBody" className="quick-actions-config__row-body">
        <div className="quick-actions-config__row-label">{actionText.label}</div>
        <div className="quick-actions-config__row-prompt">{actionText.prompt}</div>
      </div>

      <div data-bf-component="quick-actions-config" data-bf-part="rowControls" className="quick-actions-config__row-controls">
        <Switch
          checked={action.enabled}
          onChange={() => onToggle(action.id)}
        />
        <Tooltip content={t('edit.button')}>
          <IconButton
            type="button"
            size="sm"
            aria-label={t('edit.button')}
            onClick={() => onEdit(action)}
            icon={<Icon name="edit" size="xs" />}
          />
        </Tooltip>
        {canDelete && (
          <Tooltip content={t('delete.button')}>
            <IconButton
              type="button"
              size="sm"
              aria-label={t('delete.button')}
              onClick={() => onDelete(action.id)}
              className="quick-actions-config__delete-btn"
              icon={<Icon name="delete" size="lg" style={{ width: 13, height: 13 }} />}
            />
          </Tooltip>
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
      setActions(stored ?? DEFAULT_QUICK_ACTIONS);
    } catch (error) {
      log.error('Failed to load quick actions', error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const persist = useCallback(async (next: QuickAction[]) => {
    try {
      await aiExperienceConfigService.saveSettings({ quick_actions: next });
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
      <ConfigPageLayout className="quick-actions-config" data-bf-component="quick-actions-config" data-bf-part="root">
        <ConfigPageHeader title={t('page.title')} subtitle={t('page.subtitle')} />
        <ConfigPageContent>
          <ConfigLoadingState label={t('loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  const builtinActions = actions.filter(a => BUILTIN_IDS.has(a.id));
  const customActions = actions.filter(a => !BUILTIN_IDS.has(a.id));

  return (
    <ConfigPageLayout className="quick-actions-config" data-bf-component="quick-actions-config" data-bf-part="root">
      <ConfigPageHeader title={t('page.title')} subtitle={t('page.subtitle')} />

      <ConfigPageContent data-bf-component="quick-actions-config" data-bf-part="content" className="quick-actions-config__content">

        {/* ── Built-in actions ──────────────────────────────────────────── */}
        <ConfigPageSection title={t('sections.builtin.title')}>
          <div data-bf-component="quick-actions-config" data-bf-part="list" className="quick-actions-config__list">
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
          extra={
            <Button
              size="sm"
              variant="outline"
              onClick={() => setModalTarget(null)}
              leadingIcon={<Icon name="plus" size="sm" />}
            >

              {t('add.button')}
            </Button>
          }
        >
          <div data-bf-component="quick-actions-config" data-bf-part="list" className="quick-actions-config__list">
            {customActions.length === 0 ? (
              <div data-bf-component="quick-actions-config" data-bf-part="empty" data-bf-state="empty" className="quick-actions-config__empty">
                <Zap size={20} className="quick-actions-config__empty-icon" />
                <p>{t('sections.custom.empty')}</p>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => setModalTarget(null)}
                  leadingIcon={<Icon name="plus" size="sm" />}
                >

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
