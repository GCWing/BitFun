import { useId, useMemo, useRef, useState } from 'react';
import {
  Button, Checkbox, Dialog, DialogBody, DialogClose, DialogHeader, DialogHeading,
  DialogTitle, Input, OverflowText, ScrollArea, SearchField,
} from '@openbitfun/ui';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { confirmDialog } from '@/infrastructure/confirm-dialog';
import type { UserSkillGroup } from '@/infrastructure/config/types';
import { isSurfaceChangedError } from '@/infrastructure/peer-device/deviceSurface';
import type { GroupableSkill } from '@/features/skill-groups/skillGroups';
import { skillGroupErrorMessage } from './skillGroupMessages';

export interface SkillGroupDraft {
  group: UserSkillGroup;
  original: UserSkillGroup | null;
  readOnly?: boolean;
}

interface SkillGroupEditorProps {
  draft: SkillGroupDraft;
  skills: GroupableSkill[];
  catalogReady: boolean;
  saving: boolean;
  onClose: () => void;
  onCopy: () => void;
  onSave: (group: UserSkillGroup) => Promise<void>;
}

export function SkillGroupEditor({ draft, skills, catalogReady, saving, onClose, onCopy, onSave }: SkillGroupEditorProps) {
  const { t } = useI18n('scenes/skills');
  const { t: tComponents } = useI18n('components');
  const nameId = useId();
  const errorId = useId();
  const [name, setName] = useState(draft.group.name);
  const [skillKeys, setSkillKeys] = useState(draft.group.skillKeys);
  const [query, setQuery] = useState('');
  const [selectedOnly, setSelectedOnly] = useState(Boolean(draft.readOnly));
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);
  const confirmingClose = useRef(false);
  const selected = useMemo(() => new Set(skillKeys), [skillKeys]);
  const dirty = name !== draft.group.name || skillKeys.length !== draft.group.skillKeys.length
    || draft.group.skillKeys.some(key => !selected.has(key));
  const candidates = useMemo(() => {
    const catalog = new Map(skills.map(skill => [skill.key, skill]));
    // Keep absent members visible even after they are unchecked, so edits can be undone.
    return [...new Set([...catalog.keys(), ...draft.group.skillKeys])]
      .map(key => ({ key, skill: catalog.get(key) }))
      .sort((left, right) => (left.skill?.name ?? left.key).localeCompare(right.skill?.name ?? right.key)
        || left.key.localeCompare(right.key));
  }, [draft.group.skillKeys, skills]);
  const normalizedQuery = query.trim().toLowerCase();
  const visible = candidates.filter(({ key, skill }) => (
    (!(selectedOnly || draft.readOnly) || selected.has(key))
    && (!normalizedQuery || [key, skill?.name, skill?.description, skill?.sourceLabel]
      .some(value => value?.toLowerCase().includes(normalizedQuery)))
  ));

  const close = async () => {
    if (busy.current || saving || confirmingClose.current) return;
    if (dirty && !draft.readOnly) {
      confirmingClose.current = true;
      try {
        if (!await confirmDialog({
          title: t('groups.discardTitle'), message: t('groups.discardMessage'),
          confirmText: t('groups.discard'), cancelText: t('groups.keepEditing'), type: 'warning',
        })) return;
      } finally {
        confirmingClose.current = false;
      }
    }
    onClose();
  };

  const save = async () => {
    if (busy.current || saving || draft.readOnly) return;
    if (!name.trim()) { setError(t('groups.errors.nameRequired')); return; }
    busy.current = true;
    setError(null);
    try {
      await onSave({ ...draft.group, name, skillKeys });
      onClose();
    } catch (saveError) {
      if (!isSurfaceChangedError(saveError)) setError(skillGroupErrorMessage(saveError, t, 'save'));
    } finally {
      busy.current = false;
    }
  };

  return (
    <Dialog open onOpenChange={open => { if (!open) void close(); }} size="lg" data-testid="skill-group-editor">
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{draft.readOnly ? draft.group.name : t(draft.original ? 'groups.edit' : 'groups.create')}</DialogTitle>
        </DialogHeading>
        <DialogClose disabled={saving} />
      </DialogHeader>
      <DialogBody>
        <form className="skill-group-editor" data-openbitfun-scene="skills" data-openbitfun-part="groupEditor"
          onSubmit={event => { event.preventDefault(); void save(); }}>
          {!draft.readOnly && (
            <div className="skill-group-editor__field">
              <label htmlFor={nameId}>{t('groups.name')}</label>
              <Input id={nameId} size="sm" autoFocus value={name} disabled={saving}
                placeholder={t('groups.namePlaceholder')} aria-describedby={error ? errorId : undefined}
                onChange={event => { setName(event.target.value); setError(null); }} />
            </div>
          )}
          <div className="skill-group-editor__toolbar">
            <SearchField value={query} onValueChange={setQuery} size="sm"
              placeholder={t('groups.searchSkills')} aria-label={t('groups.searchSkills')}
              clearLabel={query ? tComponents('search.clear') : undefined}
              onClear={query ? () => setQuery('') : undefined} />
            {!draft.readOnly && (
              <Checkbox size="sm" checked={selectedOnly} onCheckedChange={setSelectedOnly} label={t('groups.selectedOnly')} />
            )}
          </div>
          <span className="skill-group-editor__meta">{t('groups.memberCount', { count: selected.size })}</span>
          {!catalogReady && <p role="status" className="skill-group-editor__meta">{t('groups.catalogUnavailable')}</p>}
          <ScrollArea className="skill-group-editor__members" data-openbitfun-scene="skills" data-openbitfun-part="groupMembers">
            {visible.map(({ key, skill }) => {
              const title = (
                <OverflowText className="skill-group-editor__member-title">{skill?.name ?? key}</OverflowText>
              );
              const details = skill
                ? [skill.sourceLabel ?? skill.sourceSlot, skill.description, skill.runtimeStatus].filter(Boolean).join(' · ')
                : t(catalogReady ? 'groups.unavailableMember' : 'groups.unresolvedMember');
              return (
                <div key={key} className="skill-group-editor__member" data-overflow-trigger data-openbitfun-scene="skills" data-openbitfun-part="groupMember">
                  {draft.readOnly ? (
                    <div className="skill-group-editor__identity">
                      {title}
                      <span className="skill-group-editor__member-description">{details}</span>
                    </div>
                  ) : (
                    <Checkbox className="skill-group-editor__selection" size="sm" checked={selected.has(key)} disabled={saving}
                      label={title} description={details}
                      onCheckedChange={checked => {
                        setSkillKeys(current => checked ? [...new Set([...current, key])] : current.filter(item => item !== key));
                        setError(null);
                      }} />
                  )}
                </div>
              );
            })}
            {visible.length === 0 && <p className="skill-group-editor__meta">{t('groups.noMatchingSkills')}</p>}
          </ScrollArea>
          {error && <p id={errorId} role="alert" className="skill-group-editor__error">{error}</p>}
          <div className="skill-group-editor__footer">
            <Button variant="text" size="sm" onClick={() => void close()} disabled={saving}>{t('groups.cancel')}</Button>
            {draft.readOnly ? (
              <Button variant="primary" size="sm" onClick={onCopy}>{t('groups.copy')}</Button>
            ) : (
              <Button type="submit" variant="primary" size="sm" loading={saving}>{t('groups.save')}</Button>
            )}
          </div>
        </form>
      </DialogBody>
    </Dialog>
  );
}
