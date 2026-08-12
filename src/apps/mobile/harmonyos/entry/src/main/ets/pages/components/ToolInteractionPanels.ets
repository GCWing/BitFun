import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConversationUiQuestionAnswer } from './ConversationUiModels';
import { ACCENT, CARD, INK, LINE, MUTED, PRIMARY_ACTION_TEXT, RED, SOFT } from './Theme';

@ComponentV2
export struct ToolConfirmationPanel {
  @Param toolId: string = '';
  @Param defaultInputText: string = '';
  @Param hasEditableInput: boolean = false;
  @Event onApproveTool: (toolId: string, updatedInput?: Object) => void =
    (_toolId: string, _updatedInput?: Object) => {};
  @Event onRejectTool: (toolId: string) => void = (_toolId: string) => {};
  @Local inputText: string = '';
  @Local inputError: string = '';

  aboutToAppear(): void {
    this.inputText = this.defaultInputText;
  }

  build() {
    Column({ space: 8 }) {
      if (this.hasEditableInput) {
        this.InputEditor()
      }
      Row({ space: 8 }) {
        Text(RemoteI18n.t('chat.approve'))
          .fontSize(12)
          .fontColor(PRIMARY_ACTION_TEXT)
          .textAlign(TextAlign.Center)
          .height(32)
          .layoutWeight(1)
          .backgroundColor(ACCENT)
          .borderRadius(16)
          .onClick(() => this.approve())
        Text(RemoteI18n.t('chat.reject'))
          .fontSize(12)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .height(32)
          .layoutWeight(1)
          .backgroundColor(SOFT)
          .borderRadius(16)
          .border({ width: 1, color: LINE })
          .onClick(() => this.onRejectTool(this.toolId))
      }
      .width('100%')
    }
    .width('100%')
    .padding({ left: 30 })
  }

  @Builder
  private InputEditor() {
    Column({ space: 6 }) {
      Row() {
        Text(RemoteI18n.t('chat.toolInput')).fontSize(11).fontColor(MUTED)
        Blank()
        Text(RemoteI18n.t('chat.reset'))
          .fontSize(11)
          .fontColor(MUTED)
          .onClick(() => {
            this.inputText = this.defaultInputText;
            this.inputError = '';
          })
      }
      .width('100%')
      TextArea({ placeholder: RemoteI18n.t('chat.editJsonInput'), text: this.inputText })
        .height(96)
        .fontSize(12)
        .fontColor(INK)
        .lineHeight(17)
        .backgroundColor(SOFT)
        .borderRadius(14)
        .padding(10)
        .border({ width: 1, color: this.inputError.length > 0 ? RED : LINE })
        .defaultFocus(false)
        .enabled(true)
        .onChange((value: string) => {
          this.inputText = value;
          this.inputError = '';
        })
      if (this.inputError.length > 0) {
        Text(this.inputError).fontSize(11).fontColor(RED)
      }
    }
    .width('100%')
  }

  private approve(): void {
    if (this.toolId.length === 0) {
      return;
    }
    if (!this.hasEditableInput) {
      this.onApproveTool(this.toolId);
      return;
    }
    const rawInput = this.inputText.trim();
    if (rawInput.length === 0) {
      this.inputError = RemoteI18n.t('chat.jsonObjectRequired');
      return;
    }
    try {
      const parsed = JSON.parse(rawInput) as Object;
      if (parsed === null || Array.isArray(parsed)) {
        this.inputError = RemoteI18n.t('chat.jsonObjectRequired');
        return;
      }
      this.inputError = '';
      this.onApproveTool(this.toolId, parsed);
    } catch (_err) {
      this.inputError = RemoteI18n.t('chat.jsonInvalid');
    }
  }
}

@ComponentV2
export struct ToolQuestionAnswerPanel {
  @Param toolId: string = '';
  @Param prompt: string = '';
  @Event onAnswerQuestion: (toolId: string, answers: ConversationUiQuestionAnswer) => void =
    (_toolId: string, _answers: ConversationUiQuestionAnswer) => {};
  @Local answerText: string = '';

  build() {
    Column({ space: 8 }) {
      Text(this.prompt)
        .fontSize(12)
        .lineHeight(17)
        .fontColor(INK)
        .width('100%')
      TextArea({ placeholder: RemoteI18n.t('chat.answerPlaceholder'), text: this.answerText })
        .height(78)
        .fontSize(13)
        .backgroundColor(CARD)
        .borderRadius(14)
        .padding(12)
        .border({ width: 1, color: LINE })
        .defaultFocus(false)
        .enabled(true)
        .onChange((value: string) => { this.answerText = value; })
      Row() {
        Text(RemoteI18n.t('chat.submitAnswer'))
          .fontSize(12)
          .fontColor(this.canSubmit() ? PRIMARY_ACTION_TEXT : MUTED)
          .textAlign(TextAlign.Center)
          .height(32)
          .layoutWeight(1)
          .backgroundColor(this.canSubmit() ? ACCENT : SOFT)
          .borderRadius(16)
          .onClick(() => this.submit())
      }
      .width('100%')
    }
    .width('100%')
    .padding({ left: 30 })
  }

  private canSubmit(): boolean {
    return this.toolId.length > 0 && this.answerText.trim().length > 0;
  }

  private submit(): void {
    if (!this.canSubmit()) {
      return;
    }
    const answer = this.answerText.trim();
    const answers: ConversationUiQuestionAnswer = { answer, '0': answer };
    this.onAnswerQuestion(this.toolId, answers);
    this.answerText = '';
  }
}
