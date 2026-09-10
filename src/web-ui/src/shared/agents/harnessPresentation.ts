import type { IconName } from '@openbitfun/ui';
import type { HarnessId } from './identity';

export const HARNESS_PRESENTATION: Record<HarnessId, { icon: IconName; gear: 1 | 2 | 3 | 'creative' }> = {
  Minimal: { icon: 'minimal', gear: 1 },
  Standard: { icon: 'standard', gear: 2 },
  Ultimate: { icon: 'ultimate', gear: 3 },
  Creative: { icon: 'creative', gear: 'creative' },
};
