import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const readSource = (relativePath: string): string =>
  readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8').replace(/\r\n?/g, '\n');

describe('feedback conversation contract', () => {
  it('loads earlier cached messages from a top observer and preserves the scroll anchor', () => {
    const source = readSource('./FeedbackConversationView.tsx');

    expect(source).toContain('new IntersectionObserver');
    expect(source).toContain('topSentinelRef');
    expect(source).toContain('void loadEarlier()');
    expect(source).toContain('previousTop + current.scrollHeight - previousHeight');
    expect(source).not.toContain('feedback.conversation.loadMore');
  });

  it('acks only visible admin messages while the document is foreground-visible', () => {
    const source = readSource('./FeedbackConversationView.tsx');

    expect(source).toContain("document.visibilityState !== 'visible'");
    expect(source).toContain("'[data-admin-created-at]'");
    expect(source).toContain("message.sender === 'admin'");
    expect(source).toContain('feedbackAPI.acknowledgeFeedback(record.feedbackId, requested)');
    expect(source).toContain('result.readThrough');
    expect(source).toContain('result.feedbackStatus');
  });

  it('keeps refresh accessible and renders server content as plain React text', () => {
    const source = readSource('./FeedbackConversationView.tsx');
    const mock = readSource('../../../../../../scripts/feedback-mock-server.mjs');

    expect(source).toContain('<IconButton');
    expect(source).toContain("tooltip={t('feedback.conversation.refresh')}");
    expect(source).toContain("aria-label={t('feedback.conversation.refresh')}");
    expect(source).toContain('<p>{message.content}</p>');
    expect(source).toContain("data-content-deleted={message.contentDeleted ? 'true' : undefined}");
    expect(mock).toContain('content_deleted: false');
    expect(source).not.toContain('dangerouslySetInnerHTML');
  });

  it('gates replies on consent while preserving and Unicode-truncating the draft', () => {
    const source = readSource('./FeedbackConversationView.tsx');
    const acceptPosition = source.indexOf('await accept({');
    const replyPosition = source.indexOf('await executeReply(content);', acceptPosition);

    expect(source).toContain('truncateFeedbackContent(value)');
    expect(source).toContain('feedbackContentLength(draft)');
    expect(source).toContain("status?.effectiveMode !== 'full'");
    expect(source).toContain('setShowConsent(true)');
    expect(acceptPosition).toBeGreaterThan(0);
    expect(replyPosition).toBeGreaterThan(acceptPosition);
    expect(source).toContain("setReplyError('PRIVACY_SAVE_FAILED')");
    expect(source).toContain('if (!sending) setShowConsent(false)');
  });

  it('opens the read-only privacy statement from the inline reply consent prompt', () => {
    const source = readSource('./FeedbackConversationView.tsx');
    const zh = JSON.parse(readSource('../../../locales/zh-CN/common.json')) as {
      feedback: {
        privacyStatement: string;
        reply: { consentPrefix: string; consentSuffix: string };
      };
    };

    expect(source).toContain("t('feedback.reply.consentPrefix')");
    expect(source).toContain('<PrivacyStatementLink');
    expect(source).toContain("t('feedback.reply.consentSuffix')");
    expect(source).toContain('<PrivacyStatementDialog');
    expect(source).toContain('variant="readonly"');
    expect(
      `${zh.feedback.reply.consentPrefix}${zh.feedback.privacyStatement}${zh.feedback.reply.consentSuffix}`,
    ).toBe('需要同意《隐私声明》方可发送回复。');
  });

  it('freezes reply interactions and requires confirmation before discarding a draft', () => {
    const conversation = readSource('./FeedbackConversationView.tsx');
    const dialog = readSource('./FeedbackDialog.tsx');

    expect(conversation).toContain('disabled={sending}');
    expect(conversation).toContain('confirmDisabled={sending}');
    expect(dialog).toContain("setPendingReplyExit({ kind: 'close' })");
    expect(dialog).toContain("t('feedback.reply.discardConfirm')");
    expect(dialog).toContain('setReplyResetVersion(current => current + 1)');
  });

  it('uses a reply-specific label when a failed reply is retryable', () => {
    const source = readSource('./FeedbackConversationView.tsx');
    const zh = JSON.parse(readSource('../../../locales/zh-CN/common.json')) as {
      feedback: { reply: { retry: string } };
    };

    expect(source).toContain("? t('feedback.reply.retry')");
    expect(source).not.toContain("? t('feedback.actions.retry')");
    expect(zh.feedback.reply.retry).toBe('重试发送');
  });
});
