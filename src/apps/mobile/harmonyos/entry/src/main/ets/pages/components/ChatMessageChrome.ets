import { MobileDesignGeometry, MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConversationUiMessage } from '../state/ConversationUiModels';
import { MessageImageGallery } from './ChatMessageContent';
import { ACCENT, INK, MUTED, CONTENT_ON_ACTION, STATUS_DANGER, SOFT } from './Theme';

@ComponentV2
export struct ChatTypingDots {
  @Local phase: number = 0;
  private timerId: number = 0;

  aboutToAppear(): void {
    this.timerId = setInterval(() => {
      this.phase = (this.phase + 1) % 3;
    }, 360);
  }

  aboutToDisappear(): void {
    if (this.timerId !== 0) {
      clearInterval(this.timerId);
      this.timerId = 0;
    }
  }

  build() {
    Row({ space: 5 }) {
      ForEach([0, 1, 2], (index: number) => {
        Text('•')
          .width(6)
          .height(18)
          .fontSize(MobileDesignTypography.bodyLarge.size)
          .fontColor(MUTED)
          .opacity(this.phase === index ? 1.0 : 0.34)
          .animation({ duration: 180, curve: Curve.EaseInOut })
      })
    }
    .height(24)
    .padding({ left: 2 })
  }
}

@ComponentV2
export struct ChatMessageRetryAction {
  @Param assistant: boolean = false;
  @Param retryText: string = '';
  @Event onRetry: (text: string) => void = (_text: string) => {};

  build() {
    Row({ space: 8 }) {
      Text(this.assistant ? RemoteI18n.t('generalChat.replyInterrupted') : RemoteI18n.t('chat.sendFailed'))
        .fontSize(MobileDesignTypography.labelSmall.size)
        .fontColor(STATUS_DANGER)
      Text(RemoteI18n.t('common.retry'))
        .fontSize(MobileDesignTypography.labelSmall.size)
        .fontColor(CONTENT_ON_ACTION)
        .height(28)
        .padding({ left: 10, right: 10 })
        .backgroundColor(ACCENT)
        .borderRadius(14)
        .onClick(() => this.onRetry(this.retryText))
    }
  }
}

@ComponentV2
export struct ChatUserMessageBubble {
  @Param item: ConversationUiMessage = {
    id: '',
    role: 'user',
    text: '',
    status: '',
    detail: ''
  };
  @Param showRetryAction: boolean = false;
  @Event onRetryMessage: (text: string) => void = (_text: string) => {};

  build() {
    Row() {
      Blank()
      Column({ space: 6 }) {
        if (this.visibleText().length > 0 || (this.item.images && this.item.images.length > 0)) {
          Column({ space: 8 }) {
            if (this.item.images && this.item.images.length > 0) {
              MessageImageGallery({ images: this.item.images, userStyle: true })
            }
            if (this.visibleText().length > 0) {
              Text(this.visibleText()).fontSize(MobileDesignTypography.bodyMedium.size).lineHeight(MobileDesignTypography.bodyMedium.lineHeight).fontColor(INK)
            }
          }
          .padding({
            left: MobileDesignGeometry.messageBubbleHorizontalPadding,
            right: MobileDesignGeometry.messageBubbleHorizontalPadding,
            top: MobileDesignGeometry.messageBubbleVerticalPadding,
            bottom: MobileDesignGeometry.messageBubbleVerticalPadding
          })
          .backgroundColor(SOFT)
          .borderRadius(MobileDesignGeometry.messageBubbleRadius)
          .alignItems(HorizontalAlign.Start)
        }
        if (this.item.status === 'failed' && this.showRetryAction) {
          ChatMessageRetryAction({ retryText: this.item.text, onRetry: this.onRetryMessage })
        }
      }
      .constraintSize({ maxWidth: MobileDesignGeometry.messageBubbleMaxWidth })
      .alignItems(HorizontalAlign.End)
    }
    .width('100%')
    .padding({ top: 2, bottom: 2 })
  }

  private visibleText(): string {
    const text = this.item.text.trim();
    return text === '(空消息)' || text === '(empty message)' ? '' : text;
  }
}
