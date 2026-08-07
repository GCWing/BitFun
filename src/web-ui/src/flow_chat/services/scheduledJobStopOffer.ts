/**
 * Offer to stop a session's scheduled jobs after the user cancels a turn.
 *
 * The chat stop button cancels ONE running turn. When that turn was injected
 * by a scheduled job (e.g. the continuous Issue-Fix heartbeat), cancelling is
 * almost never what the user meant: the next beat re-injects the same prompt
 * within the schedule interval. Rather than silently letting that happen —
 * or overloading the stop button with a destructive side effect — surface a
 * notification offering to disable the schedule. Disabling only stops host
 * scheduling; LoopX-side progress is untouched and Start resumes it.
 */
import { cronAPI, type CronJob } from '@/infrastructure/api/service-api/CronAPI';
import { i18nService } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('scheduledJobStopOffer');

/**
 * Snapshot the session's scheduled jobs whose turn is running RIGHT NOW.
 * Must be taken before the cancel lands: the cron subscriber clears
 * `activeTurnId` on DialogTurnCancelled, after which the association is gone.
 */
export async function snapshotRunningScheduledJobs(sessionId: string): Promise<CronJob[]> {
  try {
    const jobs = await cronAPI.listJobs({ sessionId });
    return jobs.filter((job) => job.enabled && job.state?.activeTurnId);
  } catch (error) {
    log.warn('Failed to snapshot scheduled jobs before cancel', { sessionId, error });
    return [];
  }
}

export function offerToStopScheduledJobs(jobs: CronJob[]): void {
  if (jobs.length === 0) return;
  const t = i18nService.getT();
  notificationService.info(t('flow-chat:scheduledStop.message'), {
    title: t('flow-chat:scheduledStop.title'),
    duration: 15_000,
    actions: [
      {
        label: t('flow-chat:scheduledStop.stop'),
        onClick: () => {
          for (const job of jobs) {
            void cronAPI.updateJob(job.id, { enabled: false }).catch((error) => {
              log.error('Failed to disable scheduled job', { jobId: job.id, error });
              notificationService.error(t('flow-chat:scheduledStop.stopFailed'));
            });
          }
          notificationService.success(t('flow-chat:scheduledStop.stopped'), {
            duration: 4000,
          });
        },
      },
    ],
  });
}
