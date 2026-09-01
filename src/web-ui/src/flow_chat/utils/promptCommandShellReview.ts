import { confirmDialogChoice, type ConfirmDialogChoice, type ConfirmDialogOptions } from '@/infrastructure/confirm-dialog';
import type {
  PromptCommandShellReviewDecision,
  PromptCommandShellReviewPlan,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';

type Translate = (key: string, values?: Record<string, unknown>) => string;
type ConfirmChoice = (options: ConfirmDialogOptions) => Promise<ConfirmDialogChoice>;

export async function reviewPromptCommandShell(
  plan: PromptCommandShellReviewPlan,
  translate: Translate,
  confirmChoice: ConfirmChoice = confirmDialogChoice,
): Promise<PromptCommandShellReviewDecision | null> {
  const choice = await confirmChoice({
    title: translate('chatInput.promptCommandShellReview.title'),
    message: translate('chatInput.promptCommandShellReview.message', {
      source: plan.sourceDisplayName,
      directory: plan.workingDirectory,
      shell: `${plan.shellDisplayName} (${plan.shellExecutable})`,
      count: plan.commands.length,
    }),
    type: 'warning',
    confirmText: translate('chatInput.promptCommandShellReview.runOnce'),
    secondaryText: plan.canRemember
      ? translate('chatInput.promptCommandShellReview.remember')
      : undefined,
    cancelText: translate('chatInput.promptCommandShellReview.cancel'),
    preview: plan.commands.map(command => `$ ${command}`).join('\n\n'),
  });

  if (choice === 'cancel' || (choice === 'secondary' && !plan.canRemember)) {
    return null;
  }
  return {
    planFingerprint: plan.planFingerprint,
    mode: choice === 'secondary' ? 'remember' : 'run_once',
    expectedPreferenceRevision: plan.preferenceRevision,
  };
}
