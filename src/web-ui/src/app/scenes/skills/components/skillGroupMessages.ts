import type { UseI18nReturn } from '@/infrastructure/i18n/hooks/useI18n';
import { SkillGroupError } from '@/features/skill-groups/skillGroups';

export function skillGroupErrorMessage(error: unknown, t: UseI18nReturn['t'], operation: 'load' | 'save') {
  if (error instanceof SkillGroupError) {
    switch (error.code) {
      case 'invalidConfig': return t('groups.errors.invalidConfig');
      case 'unsupportedVersion': return t('groups.errors.unsupportedVersion');
      case 'nameRequired': return t('groups.errors.nameRequired');
      case 'nameDuplicate': return t('groups.errors.nameDuplicate');
      case 'conflict': return t('groups.errors.conflict');
      case 'notReady': return t('groups.errors.notReady');
    }
  }
  return t(operation === 'load' ? 'groups.errors.loadFailed' : 'groups.errors.saveFailed');
}
