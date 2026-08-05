import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const readSource = (relativePath: string): string =>
  readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8').replace(/\r\n?/g, '\n');

describe('OpenHarmony feedback submission contract', () => {
  it('keeps other platforms on the external GitCode route', () => {
    const footer = readSource('../NavPanel/components/PersistentFooterActions.tsx');

    expect(footer).toContain("systemInfo.platform === 'openharmony'");
    expect(footer).toContain('setShowFeedback(true)');
    expect(footer).toContain("systemAPI.openExternal('https://gitcode.com/OpenHarmonyPCDeveloper/BitFun/issues')");
  });

  it('shows unread attention only for conversations that can be opened and acknowledged', () => {
    const footer = readSource('../NavPanel/components/PersistentFooterActions.tsx');
    const inbox = readSource('./FeedbackInboxView.tsx');
    const store = readSource('./feedbackInboxStore.ts');

    expect(store).toContain('export function hasActionableUnreadReply');
    expect(store).toContain('record.canOpen && record.hasNewReply');
    expect(footer).toContain('state.records.some(hasActionableUnreadReply)');
    expect(inbox).toContain('hasActionableUnreadReply(record)');
  });

  it('requires total consent before a feedback request in not-accepted mode', () => {
    const dialog = readSource('./FeedbackDialog.tsx');
    const preparePosition = dialog.indexOf('await feedbackAPI.prepareSubmission({');
    const acceptPosition = dialog.indexOf('await accept({');
    const submitPosition = dialog.indexOf('await submitPreparedFeedback();');

    expect(preparePosition).toBeGreaterThan(0);
    expect(acceptPosition).toBeGreaterThan(preparePosition);
    expect(acceptPosition).toBeGreaterThan(0);
    expect(submitPosition).toBeGreaterThan(acceptPosition);
    expect(dialog).toContain("setSubmitError('PRIVACY_SAVE_FAILED')");
    expect(dialog).toContain('return;');
  });

  it('counts the privacy checkbox as draft state and freezes close while submitting', () => {
    const dialog = readSource('./FeedbackDialog.tsx');
    const layout = readSource('../../layout/AppLayout.tsx');

    expect(dialog).toContain('category || content || includeCorrelation || privacyChecked');
    expect(dialog).toContain('if (submitting || replyState.sending) return;');
    expect(dialog).toContain('showCloseButton={!submitting && !replyState.sending}');
    expect(dialog).toContain('closeOnOverlayClick={!submitting && !replyState.sending}');
    expect(dialog).toContain('registerCriticalOperationExitGuard');
    expect(layout).toContain('await confirmCriticalOperationExit()');
  });

  it('opens the read-only privacy statement from inline consent copy', () => {
    const dialog = readSource('./FeedbackDialog.tsx');
    const privacySection = dialog.slice(
      dialog.indexOf('className="bitfun-feedback__privacy"'),
      dialog.indexOf('{submitError ?'),
    );

    expect(privacySection).toContain('<PrivacyStatementLink');
    expect(privacySection).toContain("t('feedback.privacyConsentPrefix')");
    expect(privacySection).toContain("t('feedback.privacyConsentSuffix')");
    expect(privacySection).not.toContain('feedback.viewPrivacy');
    expect(dialog).toContain('variant="readonly"');
  });

  it('shows a single completion action after capability-backed success', () => {
    const dialog = readSource('./FeedbackDialog.tsx');
    const completeView = dialog.slice(
      dialog.indexOf('className="bitfun-feedback__complete"'),
      dialog.indexOf(') : (\n          <div ref={containerRef}'),
    );

    expect(completeView).toContain("t('shared:statuses.done')");
    expect(completeView).not.toContain('openGitCode');
    expect(completeView).not.toContain('feedback-submit');
  });

  it('does not present an invalid zero-second quota retry', () => {
    const dialog = readSource('./FeedbackDialog.tsx');

    expect(dialog).toContain("feedbackError.retryAfterSeconds > 0");
    expect(dialog).toContain("error.code === 'FEEDBACK_QUOTA_EXCEEDED'");
    expect(dialog).toContain("t('feedback.errors.quotaExceeded')");
    expect(dialog).toContain('submissionRetryUntilMs = Date.now()');
    expect(dialog).toContain('submissionRetrySecondsRemaining');
    expect(dialog).toContain('const submitErrorMessage = retryWaitSeconds > 0');
    expect(dialog).toContain('if (next === 0)');
    expect(dialog).not.toContain('setQuotaBlocked(true)');
    expect(dialog).not.toContain('error.retryAfterSeconds || 0');
  });

  it('uses the feedback container width for the 840px layout threshold', () => {
    const dialog = readSource('./FeedbackDialog.tsx');
    const styles = readSource('./FeedbackDialog.scss');

    expect(dialog).toContain('new ResizeObserver');
    expect(dialog).toContain('setWideLayout(width >= 840)');
    expect(styles).toContain('&.is-wide');
    expect(styles).toContain('grid-template-columns: minmax(300px, 36%) minmax(0, 1fr)');
  });

  it('keeps the dialog below the host window controls at every supported size', () => {
    const styles = readSource('./FeedbackDialog.scss');

    expect(styles).toContain('padding: 48px 40px 32px;');
    expect(styles).toContain('width: min(960px, calc(100vw - 80px));');
    expect(styles).toContain('height: min(620px, calc(100vh - 114px));');
    expect(styles).toContain('padding: 44px 12px 12px;');
    expect(styles).toContain('max-height: calc(100vh - 56px);');
  });

  it('keeps Mock request logs limited to a fixed stage and request id', () => {
    const mock = readSource('../../../../../../scripts/feedback-mock-server.mjs');

    expect(mock).toContain('logRequestStage(requestStage(request.method, url.pathname), requestId);');
    expect(mock).toContain("return method === 'GET' ? 'message_history' : method === 'POST' ? 'reply' : 'unknown';");
    expect(mock).toContain('process.stdout.write(`${JSON.stringify({ stage, requestId })}\\n`);');
    expect(mock).not.toContain('JSON.stringify({ stage, requestId, url');
    expect(mock).not.toContain('JSON.stringify({ stage, requestId, body');
  });
});
