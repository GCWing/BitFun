import { RemoteSession, SelectedImageAttachment } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { AppSidebar } from './AppSidebar';
import { SettingsSheet } from './SettingsSheet';
import { ACCENT, CARD, INK, LINE, MUTED, PAGE_BG, SOFT, SUBTLE } from './Theme';

@Component
export struct ChatHomeView {
  @Link chatInput: string;
  @Prop sessions: RemoteSession[] = [];
  @Prop isBusy: boolean = false;
  @Prop selectedImages: SelectedImageAttachment[] = [];
  @Prop isVoiceListening: boolean = false;
  @Prop connectionState: string = 'idle';
  onEnterCode: () => void = () => {};
  onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  onPickImages: () => void = () => {};
  onVoiceInput: () => void = () => {};
  onSendGeneral: () => void = () => {};
  onSendCodeIntent: () => void = () => {};
  @State showSidebar: boolean = false;
  @State showCodeHint: boolean = false;
  @State showSettings: boolean = false;

  private openSidebar() {
    this.getUIContext().animateTo({ duration: 230, curve: Curve.EaseOut }, () => {
      this.showSidebar = true;
    });
  }

  private closeSidebar() {
    this.getUIContext().animateTo({ duration: 210, curve: Curve.EaseOut }, () => {
      this.showSidebar = false;
    });
  }

  build() {
    Stack({ alignContent: Alignment.Start }) {
      Column() {
        this.TopBar()
        Blank()
        if (!this.isVoiceListening) {
          this.PromptArea()
        }
        this.Composer()
      }
      .width('100%')
      .height('100%')
      .padding({ left: 24, right: 24, top: 0, bottom: 16 })
      .backgroundColor(PAGE_BG)

      if (this.showCodeHint) {
        this.CodeHintOverlay()
      }

      if (this.showSidebar) {
        this.SidebarLayer()
      }
    }
    .width('100%')
    .height('100%')
    .bindSheet($$this.showSettings, this.SettingsLayer(), {
      height: SheetSize.LARGE,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: false
    })
  }

  @Builder
  TopBar() {
    Row() {
      this.TopIconButton('menu', () => {
        this.openSidebar();
      })
      Text(this.isVoiceListening ? 'Thinking ›' : 'BitFun')
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .height(48)
        .padding({ left: 18, right: 18 })
        .margin({ left: 12 })
      Blank()
      this.TopIconButton('chat', () => {})
    }
    .width('100%')
    .height(52)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  PromptArea() {
    Column({ space: 0 }) {
      Column({ space: 15 }) {
        this.SuggestionRow('globe', RemoteI18n.t('chatHome.research'), RemoteI18n.t('chatHome.researchPrompt'))
        this.SuggestionRow('image', RemoteI18n.t('chatHome.image'), RemoteI18n.t('chatHome.imagePrompt'))
        this.SuggestionRow('file', RemoteI18n.t('chatHome.file'), RemoteI18n.t('chatHome.filePrompt'))
      }
      .width('100%')
      .padding({ left: 0, right: 12 })
    }
    .width('100%')
    .margin({ bottom: 18 })
  }

  @Builder
  SuggestionRow(icon: string, label: string, prompt: string) {
    Row({ space: 14 }) {
      this.SuggestionIcon(icon)
      Text(label)
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
        .layoutWeight(1)
    }
    .width('100%')
    .height(38)
    .onClick(() => {
      this.chatInput = prompt;
    })
  }

  @Builder
  Composer() {
    Column({ space: 8 }) {
      if (this.isVoiceListening) {
        this.VoiceListeningBar()
      } else {
        Row({ space: 9 }) {
          this.PlusButton()
          TextInput({ placeholder: RemoteI18n.t('chatHome.placeholder'), text: this.chatInput })
            .layoutWeight(1)
            .height(38)
            .fontSize(16)
            .fontColor(INK)
            .placeholderColor(SUBTLE)
            .backgroundColor('#00000000')
            .padding({ left: 2, right: 2 })
            .defaultFocus(false)
            .onChange((value: string) => {
              this.chatInput = value;
            })
          this.MicButton()
          this.SendButton()
        }
        .width('100%')
        .height(54)
        .padding({ left: 10, right: 10 })
        .backgroundColor(CARD)
        .borderRadius(27)
        .border({ width: 1, color: LINE })
        .shadow({ radius: 26, color: '#12000000', offsetY: 9 })
      }
    }
    .width('100%')
    .padding({ left: 18, right: 18 })
  }

  @Builder
  PlusButton() {
    Stack({ alignContent: Alignment.Center }) {
      Image($r('app.media.gpt_home_plus_glyph'))
        .width(30)
        .height(30)
        .objectFit(ImageFit.Contain)
    }
    .width(38)
    .height(38)
    .onClick(() => {
      this.onPickImages();
    })
  }

  @Builder
  VoiceListeningBar() {
    Row({ space: 14 }) {
      Button() {
        Text('')
          .width(12)
          .height(12)
          .backgroundColor(INK)
          .borderRadius(3)
      }
      .width(42)
      .height(42)
      .padding(0)
      .type(ButtonType.Circle)
      .backgroundColor('#00000000')
      .onClick(() => {
        this.onVoiceInput();
      })

      Row({ space: 3 }) {
        ForEach(this.voiceBars(), (height: number) => {
          Text('')
            .width(2.5)
            .height(height)
            .backgroundColor(height >= 17 ? '#9B9B9B' : '#D8D8D8')
            .borderRadius(2)
        })
      }
      .layoutWeight(1)
      .height(54)
      .padding({ left: 18, right: 18 })
      .backgroundColor('#F1F1F1')
      .borderRadius(27)
      .alignItems(VerticalAlign.Center)
      .justifyContent(FlexAlign.Center)

      Button() {
        Text('↑')
          .width(44)
          .height(44)
          .fontSize(29)
          .fontWeight(FontWeight.Bold)
          .fontColor(CARD)
          .textAlign(TextAlign.Center)
      }
      .width(44)
      .height(44)
      .padding(0)
      .type(ButtonType.Circle)
      .backgroundColor('#000000')
      .onClick(() => {
        this.onVoiceInput();
        if (this.chatInput.trim().length > 0) {
          this.onSendGeneral();
        }
      })
    }
    .width('100%')
    .height(64)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  MicButton() {
    Button() {
      Stack({ alignContent: Alignment.Center }) {
        Image($r('app.media.gpt_home_mic_glyph'))
          .width(27)
          .height(27)
          .objectFit(ImageFit.Contain)
      }
      .width(34)
      .height(34)
    }
    .width(34)
    .height(34)
    .padding(0)
    .type(ButtonType.Circle)
    .backgroundColor('#00000000')
    .onClick(() => {
      this.onVoiceInput();
    })
  }

  @Builder
  SendButton() {
    Button() {
      Stack({ alignContent: Alignment.Center }) {
        if (this.chatInput.trim().length > 0) {
          Text('↑')
            .width(36)
            .height(36)
            .fontSize(22)
            .fontWeight(FontWeight.Bold)
            .fontColor(CARD)
            .textAlign(TextAlign.Center)
        } else {
          Image($r('app.media.gpt_home_voice_button'))
            .width(36)
            .height(36)
            .objectFit(ImageFit.Contain)
        }
      }
      .width(36)
      .height(36)
    }
    .width(36)
    .height(36)
    .padding(0)
    .type(ButtonType.Circle)
    .backgroundColor(this.chatInput.trim().length > 0 ? ACCENT : '#00000000')
    .onClick(() => {
      if (this.chatInput.trim().length === 0) {
        this.onVoiceInput();
        return;
      }
      if (this.looksLikeCodeTask(this.chatInput)) {
        this.showCodeHint = true;
        return;
      }
      this.onSendGeneral();
    })
  }

  @Builder
  CodeHintOverlay() {
    Stack({ alignContent: Alignment.Bottom }) {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor('#55000000')
        .onClick(() => {
          this.showCodeHint = false;
        })
      Column({ space: 14 }) {
        Text(RemoteI18n.t('chatHome.codeHintTitle'))
          .fontSize(19)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
        Text(RemoteI18n.t('chatHome.codeHintBody'))
          .fontSize(14)
          .lineHeight(21)
          .fontColor(MUTED)
          .width('100%')
        Row({ space: 10 }) {
          Button(RemoteI18n.t('chatHome.enterCode'))
            .layoutWeight(1)
            .height(48)
            .fontSize(15)
            .fontWeight(FontWeight.Medium)
            .fontColor(CARD)
            .backgroundColor(ACCENT)
            .borderRadius(14)
            .onClick(() => {
              this.showCodeHint = false;
              this.onEnterCode();
            })
          Button(RemoteI18n.t('chatHome.generalAdvice'))
            .layoutWeight(1)
            .height(48)
            .fontSize(15)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
            .backgroundColor(SOFT)
            .borderRadius(14)
            .onClick(() => {
              this.showCodeHint = false;
              this.onSendCodeIntent();
            })
        }
        .width('100%')
      }
      .width('100%')
      .padding({ left: 22, right: 22, top: 22, bottom: 26 })
      .backgroundColor(CARD)
      .borderRadius({ topLeft: 22, topRight: 22 })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  SidebarLayer() {
    Stack({ alignContent: Alignment.Start }) {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor('#22000000')
        .transition(TransitionEffect.opacity(0).animation({ duration: 180, curve: Curve.EaseOut }))
        .onClick(() => {
          this.closeSidebar();
        })
      AppSidebar({
        sessions: this.sessions,
        connectionState: this.connectionState,
        onClose: () => {
          this.closeSidebar();
        },
        onNewChat: () => {
          this.chatInput = '';
          this.closeSidebar();
        },
        onEnterCode: () => {
          this.closeSidebar();
          this.onEnterCode();
        },
        onOpenSettings: () => {
          this.closeSidebar();
          this.showSettings = true;
        },
        onOpenSession: (session: RemoteSession) => {
          this.closeSidebar();
          this.onOpenSession(session);
        }
      })
        .transition(TransitionEffect.translate({ x: '-100%' })
          .combine(TransitionEffect.opacity(0.96))
          .animation({ duration: 230, curve: Curve.EaseOut }))
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  SettingsLayer() {
    SettingsSheet({
      onClose: () => {
        this.showSettings = false;
      }
    })
  }

  @Builder
  TopIconButton(kind: string, action: () => void) {
    Stack({ alignContent: Alignment.Center }) {
      if (kind === 'menu') {
        Image($r('app.media.gpt_home_menu_glyph'))
          .width(26)
          .height(26)
          .objectFit(ImageFit.Contain)
      } else {
        Image($r('app.media.gpt_home_chat_glyph'))
          .width(27)
          .height(27)
          .objectFit(ImageFit.Contain)
      }
    }
      .width(48)
      .height(48)
      .backgroundColor(CARD)
      .borderRadius(24)
      .shadow({ radius: 18, color: '#10000000', offsetY: 8 })
      .onClick(action)
  }

  @Builder
  MenuGlyph() {
    Column({ space: 7 }) {
      Text('')
        .width(26)
        .height(2)
        .backgroundColor(INK)
        .borderRadius(2)
      Text('')
        .width(26)
        .height(2)
        .backgroundColor(INK)
        .borderRadius(2)
    }
    .width(32)
    .height(22)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  ChatBubbleGlyph() {
    Stack() {
      Text('')
        .width(27)
        .height(27)
        .borderRadius(14)
        .border({ width: 1.5, color: INK })
        .position({ x: 2, y: 1 })
      Text('')
        .width(10)
        .height(7)
        .borderRadius({ bottomLeft: 7, bottomRight: 7 })
        .border({ width: { left: 1.5, bottom: 1.5 }, color: INK })
        .backgroundColor(CARD)
        .position({ x: 5, y: 24 })
      Text('')
        .width(7)
        .height(5)
        .backgroundColor(CARD)
        .position({ x: 7, y: 22 })
    }
    .width(32)
    .height(32)
  }

  @Builder
  MicrophoneGlyph(color: string) {
    Column({ space: 1 }) {
      Text('')
        .width(11)
        .height(15)
        .borderRadius(6)
        .border({ width: 1.5, color })
      Text('')
        .width(2)
        .height(4)
        .backgroundColor(color)
        .borderRadius(1)
      Text('')
        .width(12)
        .height(2)
        .backgroundColor(color)
        .borderRadius(1)
    }
    .width(34)
    .height(34)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  VoiceWaveGlyph() {
    Row({ space: 3 }) {
      this.WaveBar(10)
      this.WaveBar(15)
      this.WaveBar(21)
      this.WaveBar(15)
      this.WaveBar(8)
    }
    .width(24)
    .height(26)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  WaveBar(height: number) {
    Text('')
      .width(2.5)
      .height(height)
      .backgroundColor(CARD)
      .borderRadius(2)
  }

  @Builder
  SuggestionIcon(kind: string) {
    Stack({ alignContent: Alignment.Center }) {
      if (kind === 'globe') {
        Image($r('app.media.gpt_home_globe_glyph'))
          .width(24)
          .height(24)
          .objectFit(ImageFit.Contain)
      } else if (kind === 'image') {
        Image($r('app.media.gpt_home_image_glyph'))
          .width(24)
          .height(24)
          .objectFit(ImageFit.Contain)
      } else {
        this.FileGlyph()
      }
    }
    .width(26)
    .height(26)
  }

  @Builder
  GlobeGlyph() {
    Stack() {
      Text('')
        .width(24)
        .height(24)
        .borderRadius(12)
        .border({ width: 1.5, color: MUTED })
        .position({ x: 1, y: 1 })
      Text('')
        .width(1.5)
        .height(21)
        .backgroundColor(MUTED)
        .borderRadius(1)
        .position({ x: 12, y: 2 })
      Text('')
        .width(20)
        .height(1.5)
        .backgroundColor(MUTED)
        .borderRadius(1)
        .position({ x: 3, y: 12 })
    }
    .width(26)
    .height(26)
  }

  @Builder
  ImageGlyph() {
    Stack() {
      Text('')
        .width(24)
        .height(21)
        .borderRadius(4)
        .border({ width: 1.5, color: MUTED })
        .position({ x: 1, y: 3 })
      Text('')
        .width(5)
        .height(5)
        .borderRadius(3)
        .border({ width: 1.5, color: MUTED })
        .position({ x: 6, y: 8 })
      Text('')
        .width(15)
        .height(9)
        .borderRadius({ topLeft: 8, topRight: 8 })
        .border({ width: { top: 1.5, left: 1.5, right: 1.5 }, color: MUTED })
        .position({ x: 7, y: 15 })
    }
    .width(26)
    .height(26)
  }

  @Builder
  FileGlyph() {
    Stack() {
      Text('')
        .width(19)
        .height(23)
        .borderRadius(3)
        .border({ width: 1.5, color: MUTED })
        .position({ x: 3, y: 2 })
      Text('')
        .width(7)
        .height(7)
        .border({ width: { right: 1.5, bottom: 1.5 }, color: MUTED })
        .position({ x: 15, y: 3 })
    }
    .width(26)
    .height(26)
  }

  private looksLikeCodeTask(text: string): boolean {
    const normalized = text.toLowerCase();
    return normalized.indexOf('项目') >= 0 ||
      normalized.indexOf('启动失败') >= 0 ||
      normalized.indexOf('运行命令') >= 0 ||
      normalized.indexOf('修改代码') >= 0 ||
      normalized.indexOf('代码仓库') >= 0 ||
      normalized.indexOf('workspace') >= 0 ||
      normalized.indexOf('repo') >= 0 ||
      normalized.indexOf('run') >= 0 ||
      normalized.indexOf('debug') >= 0;
  }

  private voiceBars(): number[] {
    return [10, 12, 14, 11, 13, 15, 12, 10, 14, 13, 16, 12, 11, 14, 13, 15, 12, 10,
      15, 17, 19, 16];
  }
}
