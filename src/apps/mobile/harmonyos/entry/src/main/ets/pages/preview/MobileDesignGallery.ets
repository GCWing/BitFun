import { MobileDesignGeometry, MobileDesignTypography } from '../../generated/MobileDesignTokens';
import {
  MobilePreviewMessage,
  MobilePreviewScenario,
  MobilePreviewScenarios
} from '../../generated/MobilePreviewScenarios';
import { ComposerBar, ComposerPresentation } from '../components/ComposerBar';
import { ConversationHeader } from '../components/ConversationHeader';
import { CARD, INK, LINE, MUTED, PAGE_BG, SOFT } from '../components/Theme';

@Entry
@ComponentV2
struct MobileDesignGallery {
  @Param scenario: MobilePreviewScenario = MobilePreviewScenarios.connectedConversation;

  build() {
    Column() {
      this.PlatformLabel()
      ConversationHeader({
        title: this.scenario.headerTitle,
        subtitle: this.scenario.headerSubtitle,
        showSidebarButton: true,
        showActions: true
      })
      Column({ space: MobileDesignGeometry.messageSpacing }) {
        ForEach(this.scenario.messages, (message: MobilePreviewMessage) => {
          this.MessageBubble(message)
        }, (message: MobilePreviewMessage) => `${message.role}:${message.text}`)
      }
      .layoutWeight(1)
      .width('100%')
      .alignItems(HorizontalAlign.Start)
      .padding({
        left: MobileDesignGeometry.contentGutter,
        right: MobileDesignGeometry.contentGutter,
        top: MobileDesignGeometry.timelineTopPadding
      })
      ComposerBar({
        presentation: ComposerPresentation.Compact,
        chatInput: this.scenario.composerDraft,
        isBusy: this.scenario.streaming,
        canStop: this.scenario.streaming,
        connectionState: this.scenario.connectionPhase
      })
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  private PlatformLabel() {
    Row({ space: 8 }) {
      Text('HarmonyOS')
        .fontSize(MobileDesignTypography.labelMedium.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Text('BASELINE')
        .fontSize(MobileDesignTypography.labelSmall.size)
        .fontColor(MUTED)
        .padding({ left: 8, right: 8, top: 4, bottom: 4 })
        .backgroundColor(SOFT)
        .borderRadius(10)
      Blank()
      Text(`${this.scenario.viewportWidth} × ${this.scenario.viewportHeight}`)
        .fontSize(MobileDesignTypography.labelSmall.size)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(MobileDesignGeometry.connectionStripHeight)
    .padding({ left: MobileDesignGeometry.contentGutter, right: MobileDesignGeometry.contentGutter })
    .border({ width: { bottom: 1 }, color: LINE })
  }

  @Builder
  private MessageBubble(message: MobilePreviewMessage) {
    Row() {
      if (message.role === 'user') {
        Blank()
      }
      Text(message.text)
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontColor(INK)
        .lineHeight(MobileDesignTypography.bodyMedium.lineHeight)
        .constraintSize({ maxWidth: MobileDesignGeometry.messageBubbleMaxWidth })
        .padding({
          left: MobileDesignGeometry.messageBubbleHorizontalPadding,
          right: MobileDesignGeometry.messageBubbleHorizontalPadding,
          top: MobileDesignGeometry.messageBubbleVerticalPadding,
          bottom: MobileDesignGeometry.messageBubbleVerticalPadding
        })
        .backgroundColor(message.role === 'user' ? SOFT : CARD)
        .border({ width: 1, color: LINE })
        .borderRadius(MobileDesignGeometry.messageBubbleRadius)
      if (message.role !== 'user') {
        Blank()
      }
    }
    .width('100%')
  }
}

@Preview({
  title: 'BitFun Mobile · Compact',
  width: 390,
  height: 844
})
@ComponentV2
struct MobileDesignCompactPreview {
  build() {
    MobileDesignGallery({ scenario: MobilePreviewScenarios.connectedConversation })
  }
}

@Preview({
  title: 'BitFun Mobile · Wide',
  width: 1024,
  height: 768
})
@ComponentV2
struct MobileDesignWidePreview {
  build() {
    MobileDesignGallery({ scenario: MobilePreviewScenarios.reconnectingWide })
  }
}
