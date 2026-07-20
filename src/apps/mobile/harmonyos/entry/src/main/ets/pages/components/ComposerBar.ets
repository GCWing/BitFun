import { SelectedImageAttachment } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ACCENT, CARD, GREEN, INK, LINE, MUTED, SOFT } from './Theme';

@Component
export struct ComposerBar {
  @Link chatInput: string;
  @Link showQuickActions: boolean;
  @Prop selectedImages: SelectedImageAttachment[] = [];
  @Prop isBusy: boolean = false;
  @Prop connectionState: string = 'connected';
  @Prop isVoiceListening: boolean = false;
  onRemoveImage: (imageId: string) => void = (_imageId: string) => {};
  onSend: () => void = () => {};
  onVoiceInput: () => void = () => {};

  build() {
    Column({ space: 8 }) {
      if (this.selectedImages.length > 0) {
        this.SelectedImageStrip()
      }
      Row({ space: 9 }) {
        Text('+')
          .width(40)
          .height(40)
          .fontSize(24)
          .fontColor(this.isVoiceListening ? MUTED : INK)
          .textAlign(TextAlign.Center)
          .backgroundColor(this.isVoiceListening ? '#ECEBE8' : SOFT)
          .borderRadius(20)
          .onClick(() => {
            if (this.isVoiceListening) {
              return;
            }
            this.showQuickActions = !this.showQuickActions;
          })
        this.InputField()
        this.PrimaryActionButton()
      }
      .width('100%')
      .height(56)
      .padding({ left: 10, right: 10 })
      .backgroundColor(this.isVoiceListening ? '#FBFFFC' : CARD)
      .borderRadius(28)
      .border({ width: 1, color: this.isVoiceListening ? '#A7E6BD' : LINE })
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 8, bottom: 16 })
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
        .height(40)
        .fontSize(15)
        .fontColor(INK)
        .placeholderColor(this.isVoiceListening ? GREEN : MUTED)
        .backgroundColor('#00000000')
        .borderRadius(20)
        .padding({ left: this.isVoiceListening ? 0 : 4, right: 4 })
        .defaultFocus(false)
        .onChange((value: string) => {
          this.chatInput = value;
        })
    }
    .layoutWeight(1)
    .height(40)
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
          Text('↑')
            .width(40)
            .height(40)
            .fontSize(25)
            .fontWeight(FontWeight.Bold)
            .fontColor(this.canSend() ? CARD : MUTED)
            .textAlign(TextAlign.Center)
        } else {
          this.MicrophoneGlyph(this.canUseVoice() ? CARD : MUTED)
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
  MicrophoneGlyph(color: string) {
    Column({ space: 1 }) {
      Text('')
        .width(12)
        .height(17)
        .borderRadius(6)
        .border({ width: 2, color })
      Text('')
        .width(2)
        .height(5)
        .backgroundColor(color)
        .borderRadius(1)
      Text('')
        .width(13)
        .height(2)
        .backgroundColor(color)
        .borderRadius(1)
    }
    .width(40)
    .height(40)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
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
    return this.hasComposedContent() &&
      !this.isBusy &&
      (this.connectionState === 'connected' || this.connectionState === 'reconnecting');
  }

  private canUseVoice(): boolean {
    return !this.hasComposedContent() && !this.isBusy;
  }

  private hasComposedContent(): boolean {
    return this.chatInput.trim().length > 0 || this.selectedImages.length > 0;
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
    if (this.canSend() || this.canUseVoice()) {
      return ACCENT;
    }
    return SOFT;
  }
}
