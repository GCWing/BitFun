import { SelectedImageAttachment } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ChatComposerPolicy } from '../../services/ChatComposerPolicy';
import { ChatComposerCapabilities, REMOTE_CHAT_COMPOSER_CAPABILITIES } from './ChatComposerCapabilities';
import { ChatSurface } from './ChatSurface';
import { CARD, GREEN, INK, MUTED } from './Theme';

@Component
export struct ComposerBar {
  @Prop capabilities: ChatComposerCapabilities = REMOTE_CHAT_COMPOSER_CAPABILITIES;
  @Prop chatInput: string = '';
  @Link showQuickActions: boolean;
  @Prop selectedImages: SelectedImageAttachment[] = [];
  @Prop isBusy: boolean = false;
  @Prop connectionState: string = 'connected';
  @Prop isVoiceListening: boolean = false;
  onPickImages: () => void = () => {};
  onRemoveImage: (imageId: string) => void = (_imageId: string) => {};
  onSend: () => void = () => {};
  onVoiceInput: () => void = () => {};
  onChatInputChange: (value: string) => void = (_value: string) => {};

  build() {
    Column({ space: 8 }) {
      if (this.selectedImages.length > 0) {
        this.SelectedImageStrip()
      }
      Row({ space: 5 }) {
        if (this.shouldShowAddButton()) {
          Stack({ alignContent: Alignment.Center }) {
            this.PlusGlyph()
          }
            .width(36)
            .height(44)
            .onClick(() => {
              if (this.isVoiceListening) {
                return;
              }
              if (this.capabilities.supportsAttachments) {
                this.showQuickActions = !this.showQuickActions;
                return;
              }
              this.onPickImages();
            })
        }
        this.InputField()
        this.PrimaryActionButton()
      }
      .width('100%')
      .height(52)
      .padding({ left: 8, right: 3 })
      .backgroundColor(CARD)
      .borderRadius(26)
      .shadow({ radius: 24, color: '#18000000', offsetY: 7 })
    }
    .width('100%')
    .padding({ left: 16, right: 16, top: 8, bottom: 14 })
    .backgroundColor('#00000000')
  }

  @Builder
  InputField() {
    Row({ space: this.isVoiceListening ? 8 : 0 }) {
      if (this.isVoiceListening) {
        this.ListeningWave()
      }
      TextInput({ placeholder: this.inputPlaceholder(), text: this.chatInput })
        .layoutWeight(1)
        .height(42)
        .fontSize(16)
        .fontColor(INK)
        .placeholderColor(this.isVoiceListening ? GREEN : MUTED)
        .backgroundColor('#00000000')
        .borderRadius(20)
        .padding({ left: this.isVoiceListening ? 0 : 4, right: 4 })
        .defaultFocus(false)
        .onChange((value: string) => {
          this.onChatInputChange(value);
        })
    }
    .layoutWeight(1)
    .height(42)
    .padding({ left: this.isVoiceListening ? 12 : 0, right: 0 })
    .backgroundColor(this.isVoiceListening ? '#F0FAF4' : '#00000000')
    .borderRadius(20)
    .border({ width: this.isVoiceListening ? 1 : 0, color: this.isVoiceListening ? '#BDE8CC' : '#00000000' })
  }

  @Builder
  PrimaryActionButton() {
    Button() {
      Stack({ alignContent: Alignment.Center }) {
        if (this.isVoiceListening) {
          Text('')
            .width(13)
            .height(13)
            .backgroundColor(CARD)
            .borderRadius(3)
        } else if (this.hasComposedContent()) {
          Image($r('app.media.gpt_composer_send'))
            .width(39)
            .height(39)
            .objectFit(ImageFit.Contain)
            .opacity(this.canSend() ? 1 : 0.38)
        } else {
          this.MicrophoneGlyph()
        }
      }
      .width(40)
      .height(40)
    }
    .width(40)
    .height(40)
    .padding(0)
    .type(ButtonType.Circle)
    .backgroundColor(this.actionBackgroundColor())
    .borderRadius(20)
    .onClick(() => {
      if (this.isVoiceListening) {
        this.onVoiceInput();
        return;
      }
      if (this.canSend()) {
        this.onSend();
        return;
      }
      if (this.canUseVoice()) {
        this.onVoiceInput();
      }
    })
  }

  @Builder
  PlusGlyph() {
    Image($r('app.media.gpt_composer_plus'))
      .width(23)
      .height(23)
      .objectFit(ImageFit.Contain)
      .opacity(this.isVoiceListening ? 0.36 : 1)
  }

  @Builder
  MicrophoneGlyph() {
    Image($r('app.media.gpt_composer_mic'))
      .width(22)
      .height(27)
      .objectFit(ImageFit.Contain)
      .opacity(this.canUseVoice() ? 1 : 0.4)
  }

  @Builder
  ListeningWave() {
    Row({ space: 3 }) {
      Text('')
        .width(3)
        .height(10)
        .backgroundColor(GREEN)
        .borderRadius(2)
      Text('')
        .width(3)
        .height(18)
        .backgroundColor(GREEN)
        .borderRadius(2)
      Text('')
        .width(3)
        .height(24)
        .backgroundColor(GREEN)
        .borderRadius(2)
      Text('')
        .width(3)
        .height(14)
        .backgroundColor(GREEN)
        .borderRadius(2)
    }
    .width(22)
    .height(28)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  SelectedImageStrip() {
    Scroll() {
      Row({ space: 10 }) {
        ForEach(this.selectedImages, (image: SelectedImageAttachment) => {
          this.SelectedImageCard(image)
        }, (image: SelectedImageAttachment) => image.id)
      }
      .padding({ right: 4 })
    }
    .scrollable(ScrollDirection.Horizontal)
    .scrollBar(BarState.Off)
    .width('100%')
  }

  @Builder
  SelectedImageCard(image: SelectedImageAttachment) {
    Stack({ alignContent: Alignment.TopEnd }) {
      Image(image.data_url)
        .width(64)
        .height(64)
        .objectFit(ImageFit.Cover)
        .borderRadius(14)
      Text('×')
        .width(22)
        .height(22)
        .fontSize(14)
        .fontColor(CARD)
        .textAlign(TextAlign.Center)
        .backgroundColor('#AA222222')
        .borderRadius(11)
        .margin({ top: 4, right: 4 })
        .onClick(() => {
          this.onRemoveImage(image.id);
        })
    }
    .width(64)
    .height(64)
  }

  private canSend(): boolean {
    return ChatComposerPolicy.canSend(
      this.chatInput,
      this.selectedImages.length,
      this.isBusy,
      this.capabilities.requiresRemoteConnection,
      this.connectionState
    );
  }

  private canUseVoice(): boolean {
    return ChatComposerPolicy.canUseVoice(this.chatInput, this.selectedImages.length, this.isBusy);
  }

  private hasComposedContent(): boolean {
    return this.chatInput.trim().length > 0 || this.selectedImages.length > 0;
  }

  private shouldShowAddButton(): boolean {
    return this.capabilities.supportsAttachments || this.capabilities.surface === ChatSurface.General;
  }

  private inputPlaceholder(): string {
    return this.isVoiceListening ?
      RemoteI18n.t('chat.voiceListeningPlaceholder') :
      RemoteI18n.t('chat.inputPlaceholder');
  }

  private actionBackgroundColor(): string {
    if (this.isVoiceListening) {
      return GREEN;
    }
    return '#00000000';
  }
}
