import React, { useEffect, useMemo, useState } from 'react';
import { ArrowDown, ArrowUp, Plus, Save, ShieldCheck, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, IconButton, Input, Modal, Select, type SelectOption } from '@/component-library';
import type { PermissionEffect, PermissionRule } from '../types';
import './GlobalPermissionRulesDialog.scss';

const GLOBAL_PERMISSION_ACTION_OPTIONS: SelectOption[] = [
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

const EFFECTS: PermissionEffect[] = ['allow', 'ask', 'deny'];

interface DraftRule extends PermissionRule {
  localId: string;
}

interface GlobalPermissionRulesDialogProps {
  isOpen: boolean;
  rules: PermissionRule[];
  isSaving: boolean;
  onSave: (rules: PermissionRule[]) => Promise<boolean>;
  onClose: () => void;
}

let draftRuleSequence = 0;

function toDraftRule(rule: PermissionRule): DraftRule {
  draftRuleSequence += 1;
  return { ...rule, localId: `global-rule-${draftRuleSequence}` };
}

function toPermissionRules(rules: DraftRule[]): PermissionRule[] {
  return rules.map(({ action, resource, effect }) => ({ action, resource, effect }));
}

function rulesEqual(left: PermissionRule[], right: PermissionRule[]): boolean {
  return left.length === right.length && left.every((rule, index) => {
    const other = right[index];
    return rule.action === other.action && rule.resource === other.resource && rule.effect === other.effect;
  });
}

export const GlobalPermissionRulesDialog: React.FC<GlobalPermissionRulesDialogProps> = ({
  isOpen,
  rules,
  isSaving,
  onSave,
  onClose,
}) => {
  const { t } = useTranslation('settings/session-config');
  const [savedRules, setSavedRules] = useState<PermissionRule[]>([]);
  const [draftRules, setDraftRules] = useState<DraftRule[]>([]);
  const effectOptions = useMemo<SelectOption[]>(
    () => EFFECTS.map((effect) => ({
      value: effect,
      label: t(`permissionPolicy.globalRulesEffects.${effect}`),
    })),
    [t],
  );

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    setSavedRules(rules);
    setDraftRules(rules.map(toDraftRule));
  }, [isOpen, rules]);

  const permissionRules = useMemo(() => toPermissionRules(draftRules), [draftRules]);
  const rulesDirty = !rulesEqual(permissionRules, savedRules);
  const rulesValid = permissionRules.every((rule) => rule.action.trim() && rule.resource.trim());

  const updateDraftRule = (localId: string, update: Partial<PermissionRule>) => {
    setDraftRules((current) => current.map((rule) => (
      rule.localId === localId ? { ...rule, ...update } : rule
    )));
  };

  const moveDraftRule = (index: number, direction: -1 | 1) => {
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= draftRules.length) {
      return;
    }
    setDraftRules((current) => {
      const nextRules = [...current];
      [nextRules[index], nextRules[nextIndex]] = [nextRules[nextIndex], nextRules[index]];
      return nextRules;
    });
  };

  const handleSave = async () => {
    if (!rulesValid || isSaving) {
      return;
    }
    if (await onSave(permissionRules)) {
      setSavedRules(permissionRules);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        if (!isSaving) {
          onClose();
        }
      }}
      title={t('permissionPolicy.globalRulesDialogTitle')}
      size="xlarge"
      contentInset
      contentClassName="global-permission-rules-dialog__modal"
      overlayClassName="global-permission-rules-dialog-overlay"
    >
      <div className="global-permission-rules-dialog">
        <div className="global-permission-rules-dialog__intro">
          <ShieldCheck size={18} aria-hidden="true" />
          <p>{t('permissionPolicy.globalRulesDialogDescription')}</p>
        </div>

        <section className="global-permission-rules-dialog__section">
          <div className="global-permission-rules-dialog__section-header">
            <span>{t('permissionPolicy.globalRulesTitle')}</span>
            <Button
              size="small"
              variant="secondary"
              disabled={isSaving}
              onClick={() => setDraftRules((current) => [
                ...current,
                toDraftRule({ action: '', resource: '', effect: 'ask' }),
              ])}
            >
              <Plus size={14} />
              {t('permissionPolicy.addGlobalRule')}
            </Button>
          </div>

          {draftRules.length === 0 ? (
            <div className="global-permission-rules-dialog__empty">
              {t('permissionPolicy.globalRulesEmpty')}
            </div>
          ) : (
            <div className="global-permission-rules-dialog__rules">
              <div className="global-permission-rules-dialog__rule-heading" aria-hidden="true">
                <span>{t('permissionPolicy.globalRulesEffect')}</span>
                <span>{t('permissionPolicy.globalRulesAction')}</span>
                <span>{t('permissionPolicy.globalRulesResource')}</span>
                <span />
              </div>
              {draftRules.map((rule, index) => (
                <div key={rule.localId} className="global-permission-rules-dialog__rule-row">
                  <Select
                    size="small"
                    value={rule.effect}
                    options={effectOptions}
                    aria-label={t('permissionPolicy.globalRulesEffect')}
                    disabled={isSaving}
                    onChange={(value) => updateDraftRule(rule.localId, { effect: value as PermissionEffect })}
                  />
                  <Select
                    size="small"
                    value={rule.action}
                    options={GLOBAL_PERMISSION_ACTION_OPTIONS}
                    placeholder={t('permissionPolicy.globalRulesAction')}
                    aria-label={t('permissionPolicy.globalRulesAction')}
                    disabled={isSaving}
                    error={!rule.action.trim()}
                    onChange={(value) => updateDraftRule(rule.localId, { action: value as string })}
                  />
                  <Input
                    inputSize="small"
                    value={rule.resource}
                    placeholder={t('permissionPolicy.globalRulesResourcePlaceholder')}
                    aria-label={t('permissionPolicy.globalRulesResource')}
                    disabled={isSaving}
                    error={!rule.resource.trim()}
                    onChange={(event) => updateDraftRule(rule.localId, { resource: event.target.value })}
                  />
                  <div className="global-permission-rules-dialog__rule-actions">
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('permissionPolicy.moveGlobalRuleUp')}
                      tooltip={t('permissionPolicy.moveGlobalRuleUp')}
                      disabled={isSaving || index === 0}
                      onClick={() => moveDraftRule(index, -1)}
                    >
                      <ArrowUp size={14} />
                    </IconButton>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('permissionPolicy.moveGlobalRuleDown')}
                      tooltip={t('permissionPolicy.moveGlobalRuleDown')}
                      disabled={isSaving || index === draftRules.length - 1}
                      onClick={() => moveDraftRule(index, 1)}
                    >
                      <ArrowDown size={14} />
                    </IconButton>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('permissionPolicy.removeGlobalRule')}
                      tooltip={t('permissionPolicy.removeGlobalRule')}
                      disabled={isSaving}
                      onClick={() => setDraftRules((current) => (
                        current.filter(({ localId }) => localId !== rule.localId)
                      ))}
                    >
                      <Trash2 size={14} />
                    </IconButton>
                  </div>
                </div>
              ))}
            </div>
          )}

          {rulesDirty ? (
            <div className="global-permission-rules-dialog__footer">
              <Button
                type="button"
                variant="ghost"
                onClick={() => setDraftRules(savedRules.map(toDraftRule))}
                disabled={isSaving}
              >
                {t('permissionPolicy.discardGlobalRules')}
              </Button>
              <Button
                type="button"
                variant="primary"
                isLoading={isSaving}
                disabled={!rulesValid || isSaving}
                onClick={() => void handleSave()}
              >
                <Save size={14} />
                {t('permissionPolicy.saveGlobalRules')}
              </Button>
            </div>
          ) : null}
        </section>
      </div>
    </Modal>
  );
};
