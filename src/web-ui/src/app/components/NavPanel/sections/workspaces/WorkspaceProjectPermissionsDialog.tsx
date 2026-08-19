import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { ArrowDown, ArrowUp, Plus, Save, ShieldCheck, Trash2 } from 'lucide-react';
import { Button, IconButton, Input, Modal, Select, confirmDanger, type SelectOption } from '@/component-library';
import {
  permissionAPI,
  type PermissionGrant,
  type ProjectPermissionEffect,
  type ProjectPermissionRule,
} from '@/infrastructure/api/service-api/PermissionAPI';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import type { WorkspaceInfo } from '@/shared/types';
import { createLogger } from '@/shared/utils/logger';
import './WorkspaceProjectPermissionsDialog.scss';

const log = createLogger('WorkspaceProjectPermissionsDialog');

const PROJECT_PERMISSION_ACTION_OPTIONS: SelectOption[] = [
  { value: '*', label: '*' },
  { value: 'read', label: 'read' },
  { value: 'edit', label: 'edit' },
  { value: 'bash', label: 'bash' },
  { value: 'git', label: 'git' },
  { value: 'websearch', label: 'websearch' },
  { value: 'webfetch', label: 'webfetch' },
  { value: 'task', label: 'task' },
  { value: 'skill', label: 'skill' },
  { value: 'mcp', label: 'mcp' },
  { value: 'computer_use', label: 'computer_use' },
  { value: 'custom_tool', label: 'custom_tool' },
  { value: 'external_directory', label: 'external_directory' },
];
const EFFECTS: ProjectPermissionEffect[] = ['allow', 'ask', 'deny'];

let draftRuleSequence = 0;

interface DraftRule extends ProjectPermissionRule {
  localId: string;
}

interface WorkspaceProjectPermissionsDialogProps {
  workspace: WorkspaceInfo;
  isOpen: boolean;
  onClose: () => void;
}

function toDraftRule(rule: ProjectPermissionRule): DraftRule {
  draftRuleSequence += 1;
  return { ...rule, localId: `project-rule-${draftRuleSequence}` };
}

function toProjectRules(rules: DraftRule[]): ProjectPermissionRule[] {
  return rules.map(({ action, resource, effect }) => ({ action, resource, effect }));
}

function rulesEqual(left: ProjectPermissionRule[], right: ProjectPermissionRule[]): boolean {
  return left.length === right.length && left.every((rule, index) => {
    const other = right[index];
    return rule.action === other.action && rule.resource === other.resource && rule.effect === other.effect;
  });
}

function arraysEqual(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

interface SensitiveResourceSectionProps {
  title: string;
  description: string;
  emptyText: string;
  addLabel: string;
  placeholder: string;
  removeLabel: string;
  values: string[];
  dirty: boolean;
  loading: boolean;
  busy: boolean;
  revisionAvailable: boolean;
  saving: boolean;
  onValuesChange: (values: string[]) => void;
  onSave: () => void;
  onDiscard: () => void;
}

const SensitiveResourceSection: React.FC<SensitiveResourceSectionProps> = ({
  title,
  description,
  emptyText,
  addLabel,
  placeholder,
  removeLabel,
  values,
  dirty,
  loading,
  busy,
  revisionAvailable,
  saving,
  onValuesChange,
  onSave,
  onDiscard,
}) => {
  const { t } = useI18n('settings/session-config');
  return (
  <section data-bf-component="workspace-project-permissions-dialog" data-bf-part="section" className="workspace-project-permissions-dialog__section workspace-project-permissions-dialog__sensitive-section">
    <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="sectionHeader" className="workspace-project-permissions-dialog__section-header">
      <span>{title}</span>
      <Button
        size="small"
        variant="secondary"
        disabled={busy || !revisionAvailable}
        onClick={() => onValuesChange([...values, ''])}
      >
        <Plus size={14} />
        {addLabel}
      </Button>
    </div>

    <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="sensitiveBody" className="workspace-project-permissions-dialog__sensitive-body">
      <p className="workspace-project-permissions-dialog__sensitive-description">
        {description}
      </p>

      {loading ? (
        <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="empty" className="workspace-project-permissions-dialog__empty">{t('loading.text')}</div>
      ) : values.length === 0 ? (
        <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="empty" className="workspace-project-permissions-dialog__empty">{emptyText}</div>
      ) : (
        <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="sensitiveResources" className="workspace-project-permissions-dialog__sensitive-resources">
          {values.map((value, index) => (
            <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="sensitiveResource" key={index} className="workspace-project-permissions-dialog__sensitive-row">
              <Input
                inputSize="small"
                value={value}
                placeholder={placeholder}
                aria-label={placeholder}
                disabled={busy}
                onChange={(event) => onValuesChange(values.map((item, itemIndex) => (itemIndex === index ? event.target.value : item)))}
              />
              <IconButton
                type="button"
                size="small"
                variant="ghost"
                aria-label={removeLabel}
                tooltip={removeLabel}
                disabled={busy}
                onClick={() => onValuesChange(values.filter((_, itemIndex) => itemIndex !== index))}
              >
                <Trash2 size={14} />
              </IconButton>
            </div>
          ))}
        </div>
      )}

      {dirty ? (
        <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="footer" className="workspace-project-permissions-dialog__footer">
          <Button type="button" variant="ghost" onClick={onDiscard} disabled={busy}>
            {t('projectPermissions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            isLoading={saving}
            disabled={!revisionAvailable || busy}
            onClick={onSave}
          >
            <Save size={14} />
            {t('projectPermissions.saveRules')}
          </Button>
        </div>
      ) : null}
    </div>
  </section>
  );
};

export const WorkspaceProjectPermissionsDialog: React.FC<WorkspaceProjectPermissionsDialogProps> = ({
  workspace,
  isOpen,
  onClose,
}) => {
  const { t, formatDate } = useI18n('settings/session-config');
  const [permissionGrants, setPermissionGrants] = useState<PermissionGrant[]>([]);
  const [grantsLoading, setGrantsLoading] = useState(false);
  const [rulesLoading, setRulesLoading] = useState(false);
  const [rulesSaving, setRulesSaving] = useState(false);
  const [mutationKey, setMutationKey] = useState<string | null>(null);
  const [savedRules, setSavedRules] = useState<ProjectPermissionRule[]>([]);
  const [draftRules, setDraftRules] = useState<DraftRule[]>([]);
  const [savedSensitiveResources, setSavedSensitiveResources] = useState<string[]>([]);
  const [draftSensitiveResources, setDraftSensitiveResources] = useState<string[]>([]);
  const [savedWriteSensitiveResources, setSavedWriteSensitiveResources] = useState<string[]>([]);
  const [draftWriteSensitiveResources, setDraftWriteSensitiveResources] = useState<string[]>([]);
  const [rulesRevision, setRulesRevision] = useState<string | null>(null);
  const effectOptions = useMemo<SelectOption[]>(
    () => EFFECTS.map((effect) => ({
      value: effect,
      label: t(`projectPermissions.effects.${effect}`),
    })),
    [t],
  );

  const loadGrants = useCallback(async () => {
    setGrantsLoading(true);
    try {
      setPermissionGrants(await permissionAPI.listProjectGrants(workspace.id));
    } catch (error) {
      log.error('Failed to load project permission grants', { workspaceId: workspace.id, error });
      notificationService.error(t('projectPermissions.grantsLoadFailed'));
    } finally {
      setGrantsLoading(false);
    }
  }, [t, workspace.id]);

  const loadRules = useCallback(async () => {
    setRulesLoading(true);
    try {
      const response = await permissionAPI.getProjectRules(workspace.id);
      setSavedRules(response.rules);
      setDraftRules(response.rules.map(toDraftRule));
      setSavedSensitiveResources(response.sensitiveResources);
      setDraftSensitiveResources(response.sensitiveResources);
      setSavedWriteSensitiveResources(response.writeSensitiveResources);
      setDraftWriteSensitiveResources(response.writeSensitiveResources);
      setRulesRevision(response.revision);
    } catch (error) {
      log.error('Failed to load project permission rules', { workspaceId: workspace.id, error });
      setRulesRevision(null);
      notificationService.error(t('projectPermissions.rulesLoadFailed'));
    } finally {
      setRulesLoading(false);
    }
  }, [t, workspace.id]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    void Promise.all([loadGrants(), loadRules()]);
  }, [isOpen, loadGrants, loadRules]);

  const handleRemovePermissionGrant = async (grant: PermissionGrant) => {
    const confirmed = await confirmDanger(
      t('projectPermissions.removeGrantTitle'),
      t('projectPermissions.removeGrantMessage', { action: grant.action, resource: grant.resource }),
      { confirmText: t('projectPermissions.removeGrantConfirm') },
    );
    if (!confirmed) {
      return;
    }

    const key = `${grant.action}\n${grant.resource}`;
    setMutationKey(key);
    try {
      await permissionAPI.removeProjectGrant(workspace.id, grant);
      await loadGrants();
      notificationService.success(t('projectPermissions.removeGrantSuccess'));
    } catch (error) {
      log.error('Failed to remove project permission grant', {
        workspaceId: workspace.id,
        action: grant.action,
        resource: grant.resource,
        error,
      });
      notificationService.error(t('projectPermissions.removeGrantFailed'));
    } finally {
      setMutationKey(null);
    }
  };

  const handleClearPermissionGrants = async () => {
    if (permissionGrants.length === 0) {
      return;
    }

    const confirmed = await confirmDanger(
      t('projectPermissions.clearGrantsTitle'),
      t('projectPermissions.clearGrantsMessage'),
      { confirmText: t('projectPermissions.clearGrantsConfirm') },
    );
    if (!confirmed) {
      return;
    }

    setMutationKey('*');
    try {
      await permissionAPI.clearProjectGrants(workspace.id);
      await loadGrants();
      notificationService.success(t('projectPermissions.clearGrantsSuccess'));
    } catch (error) {
      log.error('Failed to clear project permission grants', { workspaceId: workspace.id, error });
      notificationService.error(t('projectPermissions.clearGrantsFailed'));
    } finally {
      setMutationKey(null);
    }
  };

  const updateDraftRule = (localId: string, update: Partial<ProjectPermissionRule>) => {
    setDraftRules((rules) => rules.map((rule) => (rule.localId === localId ? { ...rule, ...update } : rule)));
  };

  const moveDraftRule = (index: number, direction: -1 | 1) => {
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= draftRules.length) {
      return;
    }
    setDraftRules((rules) => {
      const nextRules = [...rules];
      [nextRules[index], nextRules[nextIndex]] = [nextRules[nextIndex], nextRules[index]];
      return nextRules;
    });
  };

  const isMutationRunning = mutationKey !== null;
  const projectRules = useMemo(() => toProjectRules(draftRules), [draftRules]);
  const rulesDirty = !rulesEqual(projectRules, savedRules);
  const readSensitiveResourcesDirty = !arraysEqual(draftSensitiveResources, savedSensitiveResources);
  const writeSensitiveResourcesDirty = !arraysEqual(draftWriteSensitiveResources, savedWriteSensitiveResources);
  const rulesValid = projectRules.every((rule) => rule.action.trim() && rule.resource.trim());
  const isBusy = grantsLoading || rulesLoading || rulesSaving || isMutationRunning;

  const applyPermissionSaveResponse = (response: {
    rules: ProjectPermissionRule[];
    sensitiveResources: string[];
    writeSensitiveResources: string[];
    revision: string;
  }) => {
    setSavedRules(response.rules);
    setDraftRules(response.rules.map(toDraftRule));
    setSavedSensitiveResources(response.sensitiveResources);
    setDraftSensitiveResources(response.sensitiveResources);
    setSavedWriteSensitiveResources(response.writeSensitiveResources);
    setDraftWriteSensitiveResources(response.writeSensitiveResources);
    setRulesRevision(response.revision);
  };

  const handleSaveRules = async () => {
    if (!rulesValid || rulesRevision === null) {
      return;
    }

    setRulesSaving(true);
    try {
      const response = await permissionAPI.saveProjectRules(
        workspace.id,
        projectRules,
        savedSensitiveResources,
        savedWriteSensitiveResources,
        rulesRevision,
      );
      applyPermissionSaveResponse(response);
      notificationService.success(t('projectPermissions.rulesSaveSuccess'));
    } catch (error) {
      log.error('Failed to save project permission rules', { workspaceId: workspace.id, error });
      notificationService.error(
        error instanceof Error && error.message.includes('changed outside BitFun')
          ? t('projectPermissions.rulesConflict')
          : t('projectPermissions.rulesSaveFailed'),
      );
    } finally {
      setRulesSaving(false);
    }
  };

  const handleSaveReadSensitiveResources = async () => {
    if (rulesRevision === null) {
      return;
    }

    // Trim and drop empty rows before persisting; an unfinished draft row is
    // not an error, it is just input the user never filled.
    const normalized = draftSensitiveResources
      .map((value) => value.trim())
      .filter((value) => value.length > 0);

    setRulesSaving(true);
    try {
      const response = await permissionAPI.saveProjectRules(
        workspace.id,
        savedRules,
        normalized,
        savedWriteSensitiveResources,
        rulesRevision,
      );
      applyPermissionSaveResponse(response);
      notificationService.success(t('projectPermissions.rulesSaveSuccess'));
    } catch (error) {
      log.error('Failed to save project sensitive resources', { workspaceId: workspace.id, error });
      notificationService.error(
        error instanceof Error && error.message.includes('changed outside BitFun')
          ? t('projectPermissions.rulesConflict')
          : t('projectPermissions.rulesSaveFailed'),
      );
    } finally {
      setRulesSaving(false);
    }
  };

  const handleSaveWriteSensitiveResources = async () => {
    if (rulesRevision === null) {
      return;
    }

    const normalized = draftWriteSensitiveResources
      .map((value) => value.trim())
      .filter((value) => value.length > 0);

    setRulesSaving(true);
    try {
      const response = await permissionAPI.saveProjectRules(
        workspace.id,
        savedRules,
        savedSensitiveResources,
        normalized,
        rulesRevision,
      );
      applyPermissionSaveResponse(response);
      notificationService.success(t('projectPermissions.rulesSaveSuccess'));
    } catch (error) {
      log.error('Failed to save project write-sensitive resources', { workspaceId: workspace.id, error });
      notificationService.error(
        error instanceof Error && error.message.includes('changed outside BitFun')
          ? t('projectPermissions.rulesConflict')
          : t('projectPermissions.rulesSaveFailed'),
      );
    } finally {
      setRulesSaving(false);
    }
  };

  const handleDiscardRules = () => {
    setDraftRules(savedRules.map(toDraftRule));
  };

  const handleDiscardReadSensitiveResources = () => {
    setDraftSensitiveResources(savedSensitiveResources);
  };

  const handleDiscardWriteSensitiveResources = () => {
    setDraftWriteSensitiveResources(savedWriteSensitiveResources);
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        if (!isBusy) {
          onClose();
        }
      }}
      title={workspace.name}
      size="xlarge"
      contentInset
      contentClassName="workspace-project-permissions-dialog__modal"
      overlayClassName="workspace-project-permissions-dialog-overlay"
    >
      <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="root" className="workspace-project-permissions-dialog">
        <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="intro" className="workspace-project-permissions-dialog__intro">
          <ShieldCheck size={18} aria-hidden="true" />
          <p>{t('projectPermissions.description')}</p>
        </div>

        <section data-bf-component="workspace-project-permissions-dialog" data-bf-part="section" className="workspace-project-permissions-dialog__section">
          <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="sectionHeader" className="workspace-project-permissions-dialog__section-header">
            <span>{t('projectPermissions.grantsTitle')}</span>
            {permissionGrants.length > 0 ? (
              <Button
                size="small"
                variant="secondary"
                onClick={() => void handleClearPermissionGrants()}
                disabled={isBusy}
              >
                <Trash2 size={14} />
                {t('projectPermissions.clearGrants')}
              </Button>
            ) : null}
          </div>

          <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="grants" className="workspace-project-permissions-dialog__grants">
            {grantsLoading && permissionGrants.length === 0 ? (
              <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="empty" className="workspace-project-permissions-dialog__empty">{t('loading.text')}</div>
            ) : permissionGrants.length === 0 ? (
              <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="empty" className="workspace-project-permissions-dialog__empty">{t('projectPermissions.grantsEmpty')}</div>
            ) : permissionGrants.map((grant) => {
              const key = `${grant.action}\n${grant.resource}`;
              return (
                <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="grant" key={key} className="workspace-project-permissions-dialog__grant-row">
                  <div className="workspace-project-permissions-dialog__grant-copy">
                    <code>{grant.action}</code>
                    <code title={grant.resource}>{grant.resource}</code>
                    <span>{formatDate(grant.createdAtMs, { dateStyle: 'medium', timeStyle: 'short' })}</span>
                  </div>
                  <IconButton
                    type="button"
                    size="small"
                    variant="ghost"
                    aria-label={t('projectPermissions.removeGrant')}
                    tooltip={t('projectPermissions.removeGrant')}
                    disabled={isBusy}
                    onClick={() => void handleRemovePermissionGrant(grant)}
                  >
                    <Trash2 size={14} />
                  </IconButton>
                </div>
              );
            })}
          </div>
        </section>

        <section data-bf-component="workspace-project-permissions-dialog" data-bf-part="section" className="workspace-project-permissions-dialog__section workspace-project-permissions-dialog__rules-section">
          <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="sectionHeader" className="workspace-project-permissions-dialog__section-header">
            <span>{t('projectPermissions.rulesTitle')}</span>
            <Button
              size="small"
              variant="secondary"
              disabled={isBusy || rulesRevision === null}
              onClick={() => setDraftRules((rules) => [...rules, toDraftRule({ action: '', resource: '', effect: 'ask' })])}
            >
              <Plus size={14} />
              {t('projectPermissions.addRule')}
            </Button>
          </div>

          {rulesLoading ? (
            <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="empty" className="workspace-project-permissions-dialog__empty">{t('loading.text')}</div>
          ) : draftRules.length === 0 ? (
            <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="empty" className="workspace-project-permissions-dialog__empty">{t('projectPermissions.rulesEmpty')}</div>
          ) : (
            <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="rules" className="workspace-project-permissions-dialog__rules">
              <div className="workspace-project-permissions-dialog__rule-heading" aria-hidden="true">
                <span>{t('projectPermissions.effect')}</span>
                <span>{t('projectPermissions.action')}</span>
                <span>{t('projectPermissions.resource')}</span>
                <span />
              </div>
              {draftRules.map((rule, index) => (
                <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="rule" key={rule.localId} className="workspace-project-permissions-dialog__rule-row">
                  <Select
                    size="small"
                    value={rule.effect}
                    options={effectOptions}
                    aria-label={t('projectPermissions.effect')}
                    disabled={isBusy}
                    onChange={(value) => updateDraftRule(rule.localId, { effect: value as ProjectPermissionEffect })}
                  />
                  <Select
                    size="small"
                    value={rule.action}
                    options={PROJECT_PERMISSION_ACTION_OPTIONS}
                    placeholder={t('projectPermissions.action')}
                    aria-label={t('projectPermissions.action')}
                    disabled={isBusy}
                    error={!rule.action.trim()}
                    onChange={(value) => updateDraftRule(rule.localId, { action: value as string })}
                  />
                  <Input
                    inputSize="small"
                    value={rule.resource}
                    placeholder={t('projectPermissions.resourcePlaceholder')}
                    aria-label={t('projectPermissions.resource')}
                    disabled={isBusy}
                    error={!rule.resource.trim()}
                    onChange={(event) => updateDraftRule(rule.localId, { resource: event.target.value })}
                  />
                  <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="ruleActions" className="workspace-project-permissions-dialog__rule-actions">
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('projectPermissions.moveRuleUp')}
                      tooltip={t('projectPermissions.moveRuleUp')}
                      disabled={isBusy || index === 0}
                      onClick={() => moveDraftRule(index, -1)}
                    >
                      <ArrowUp size={14} />
                    </IconButton>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('projectPermissions.moveRuleDown')}
                      tooltip={t('projectPermissions.moveRuleDown')}
                      disabled={isBusy || index === draftRules.length - 1}
                      onClick={() => moveDraftRule(index, 1)}
                    >
                      <ArrowDown size={14} />
                    </IconButton>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('projectPermissions.removeRule')}
                      tooltip={t('projectPermissions.removeRule')}
                      disabled={isBusy}
                      onClick={() => setDraftRules((rules) => rules.filter(({ localId }) => localId !== rule.localId))}
                    >
                      <Trash2 size={14} />
                    </IconButton>
                  </div>
                </div>
              ))}
            </div>
          )}

          {rulesDirty ? (
            <div data-bf-component="workspace-project-permissions-dialog" data-bf-part="footer" className="workspace-project-permissions-dialog__footer">
              <Button type="button" variant="ghost" onClick={handleDiscardRules} disabled={isBusy}>
                {t('projectPermissions.cancel')}
              </Button>
              <Button
                type="button"
                variant="primary"
                isLoading={rulesSaving}
                disabled={!rulesValid || rulesRevision === null || isBusy}
                onClick={() => void handleSaveRules()}
              >
                <Save size={14} />
                {t('projectPermissions.saveRules')}
              </Button>
            </div>
          ) : null}
        </section>

        <SensitiveResourceSection
          title={t('projectPermissions.sensitiveResourcesTitle')}
          description={t('projectPermissions.sensitiveResourcesDescription')}
          emptyText={t('projectPermissions.sensitiveResourcesEmpty')}
          addLabel={t('projectPermissions.addSensitiveResource')}
          placeholder={t('projectPermissions.sensitiveResourcePlaceholder')}
          removeLabel={t('projectPermissions.removeSensitiveResource')}
          values={draftSensitiveResources}
          dirty={readSensitiveResourcesDirty}
          loading={rulesLoading}
          busy={isBusy}
          revisionAvailable={rulesRevision !== null}
          saving={rulesSaving}
          onValuesChange={setDraftSensitiveResources}
          onSave={() => void handleSaveReadSensitiveResources()}
          onDiscard={handleDiscardReadSensitiveResources}
        />

        <SensitiveResourceSection
          title={t('projectPermissions.writeSensitiveResourcesTitle')}
          description={t('projectPermissions.writeSensitiveResourcesDescription')}
          emptyText={t('projectPermissions.writeSensitiveResourcesEmpty')}
          addLabel={t('projectPermissions.addWriteSensitiveResource')}
          placeholder={t('projectPermissions.writeSensitiveResourcePlaceholder')}
          removeLabel={t('projectPermissions.removeWriteSensitiveResource')}
          values={draftWriteSensitiveResources}
          dirty={writeSensitiveResourcesDirty}
          loading={rulesLoading}
          busy={isBusy}
          revisionAvailable={rulesRevision !== null}
          saving={rulesSaving}
          onValuesChange={setDraftWriteSensitiveResources}
          onSave={() => void handleSaveWriteSensitiveResources()}
          onDiscard={handleDiscardWriteSensitiveResources}
        />
      </div>
    </Modal>
  );
};

export default WorkspaceProjectPermissionsDialog;
