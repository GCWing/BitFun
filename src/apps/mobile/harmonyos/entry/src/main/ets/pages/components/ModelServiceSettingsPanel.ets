import { KeyboardAvoidMode } from '@kit.ArkUI';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteModelCatalog, RemoteModelConfig } from '../../model/RemoteModels';
import { GENERAL_CHAT_LOCAL_MODEL_ID } from '../../services/general-chat/GeneralChatConfigStore';
import { ACCENT, CARD, GREEN, INK, LINE, MUTED, PRIMARY_ACTION_TEXT, RED, SOFT, SUBTLE } from './Theme';

@ComponentV2
export struct ModelServiceSettingsPanel {
  private readonly contentScroller: Scroller = new Scroller();
  private focusScrollTimerId: number = 0;
  private blurResetTimerId: number = 0;
  private previousKeyboardAvoidMode: KeyboardAvoidMode = KeyboardAvoidMode.OFFSET;
  @Param apiUrl: string = '';
  @Param modelName: string = '';
  @Param hasApiKey: boolean = false;
  @Param modelCatalog: RemoteModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @Param selectedModelId: string = '';
  @Event onClose: () => void = () => {};
  @Event onSaved: (apiUrl: string, modelName: string, hasApiKey: boolean) => void = () => {};
  @Event onTest: (
    apiUrl: string,
    apiKey: string,
    modelName: string,
    clearApiKey: boolean
  ) => Promise<string> = async (
    _apiUrl: string,
    _apiKey: string,
    _modelName: string,
    _clearApiKey: boolean
  ) => '';
  @Event onSave: (
    apiUrl: string,
    apiKey: string,
    modelName: string,
    clearApiKey: boolean
  ) => Promise<string> = async (
    _apiUrl: string,
    _apiKey: string,
    _modelName: string,
    _clearApiKey: boolean
  ) => '';
  @Local draftApiUrl: string = '';
  @Local draftApiKey: string = '';
  @Local draftModelName: string = '';
  @Local clearApiKey: boolean = false;
  @Local isSaving: boolean = false;
  @Local isTesting: boolean = false;
  @Local feedbackText: string = '';
  @Local feedbackIsError: boolean = false;
  @Local focusedFieldKind: string = '';
  @Local showLocalEditor: boolean = false;

  aboutToAppear(): void {
    this.previousKeyboardAvoidMode = this.getUIContext().getKeyboardAvoidMode();
    this.getUIContext().setKeyboardAvoidMode(KeyboardAvoidMode.RESIZE);
    this.draftApiUrl = this.apiUrl;
    this.draftModelName = this.modelName;
    this.draftApiKey = '';
    this.clearApiKey = false;
    this.isTesting = false;
    this.feedbackText = '';
    this.feedbackIsError = false;
    this.showLocalEditor = false;
  }

  aboutToDisappear(): void {
    this.clearFocusScrollTimer();
    this.clearBlurResetTimer();
    this.getUIContext().setKeyboardAvoidMode(this.previousKeyboardAvoidMode);
  }

  build() {
    Stack({ alignContent: Alignment.Bottom }) {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor('#33000000')
        .onClick(() => {
          if (!this.isSaving && !this.isTesting) {
            this.onClose();
          }
        })

      Column({ space: 0 }) {
        this.Header()
        if (this.showLocalEditor) {
          this.LocalEditor()
        } else {
          this.ModelOverview()
        }
      }
      .width('100%')
      .height('78%')
      .backgroundColor(CARD)
      .borderRadius({ topLeft: 28, topRight: 28 })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  Header() {
    Row() {
      if (this.showLocalEditor) {
        Button() {
          SymbolGlyph($r('sys.symbol.chevron_left'))
            .fontSize(20)
            .fontColor([INK])
        }
        .width(42)
        .height(42)
        .padding(0)
        .type(ButtonType.Circle)
        .backgroundColor(SOFT)
        .margin({ right: 12 })
        .onClick(() => {
          if (!this.isSaving && !this.isTesting) {
            this.showLocalEditor = false;
            this.resetEditorScroll();
          }
        })
      }
      Text(RemoteI18n.t(this.showLocalEditor ?
        'settings.modelService.localTitle' : 'settings.modelService.manageTitle'))
        .fontSize(21)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
      Blank()
      Button() {
        SymbolGlyph($r('sys.symbol.xmark'))
          .fontSize(20)
          .fontColor([INK])
      }
      .width(42)
      .height(42)
      .padding(0)
      .type(ButtonType.Circle)
      .backgroundColor(SOFT)
      .onClick(() => {
        if (!this.isSaving && !this.isTesting) {
          this.onClose();
        }
      })
    }
    .width('100%')
    .height(66)
    .padding({ left: 22, right: 18 })
    .border({ width: { bottom: 1 }, color: LINE })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  ModelOverview() {
    Scroll() {
      Column({ space: 20 }) {
        Column({ space: 8 }) {
          this.SectionHeader(RemoteI18n.t('settings.modelService.currentModel'), '')
          this.CurrentModelRow()
        }
        .width('100%')

        Column({ space: 8 }) {
          this.SectionHeader(RemoteI18n.t('settings.modelService.accountModels'), '')
          this.AccountModelsSummaryRow()
        }
        .width('100%')

        Column({ space: 8 }) {
          this.SectionHeader(RemoteI18n.t('settings.modelService.localModel'), '')
          this.LocalModelRow()
        }
        .width('100%')
      }
      .width('100%')
      .padding({ left: 22, right: 22, top: 18, bottom: 30 })
      .justifyContent(FlexAlign.Start)
    }
    .width('100%')
    .layoutWeight(1)
    .scrollBar(BarState.Off)
  }

  @Builder
  LocalEditor() {
    Scroll(this.contentScroller) {
      Column({ space: 20 }) {
        this.ConfigField(
          RemoteI18n.t('settings.modelService.apiUrl'),
          RemoteI18n.t('settings.modelService.apiUrlPlaceholder'),
          'url'
        )
        this.ApiKeyField()
        this.ConfigField(
          RemoteI18n.t('settings.modelService.modelName'),
          RemoteI18n.t('settings.modelService.modelPlaceholder'),
          'model'
        )
        Row({ space: 12 }) {
          Button(this.isTesting ? RemoteI18n.t('common.loading') : RemoteI18n.t('settings.modelService.testConnection'))
            .layoutWeight(1)
            .height(50)
            .fontSize(16)
            .fontWeight(FontWeight.Medium)
            .fontColor(INK)
            .backgroundColor(this.isTesting ? MUTED : SOFT)
            .borderRadius(25)
            .enabled(!this.isSaving && !this.isTesting && this.hasTestableApiKey())
            .onClick(() => {
              this.testConnection();
            })
          Button(this.isSaving ? RemoteI18n.t('common.loading') : RemoteI18n.t('common.save'))
            .layoutWeight(1)
            .height(50)
            .fontSize(16)
            .fontWeight(FontWeight.Medium)
            .fontColor(PRIMARY_ACTION_TEXT)
            .backgroundColor(this.isSaving ? MUTED : ACCENT)
            .borderRadius(25)
            .enabled(!this.isSaving && !this.isTesting)
            .onClick(() => {
              this.save();
            })
        }
        .width('100%')

        if (!this.hasTestableApiKey()) {
          Text(RemoteI18n.t('settings.modelService.testNeedsKey'))
            .fontSize(12)
            .lineHeight(18)
            .fontColor(SUBTLE)
            .width('100%')
        }

        if (this.feedbackText.length > 0) {
          Text(this.feedbackText)
            .fontSize(13)
            .lineHeight(19)
            .fontColor(this.feedbackIsError ? RED : GREEN)
            .width('100%')
        }
      }
      .width('100%')
      .padding({
        left: 22,
        right: 22,
        top: 18,
        bottom: this.focusedFieldKind.length > 0 ? 360 : 30
      })
      .justifyContent(FlexAlign.Start)
    }
    .width('100%')
    .layoutWeight(1)
    .scrollBar(BarState.Off)
  }

  @Builder
  SectionHeader(title: string, detail: string) {
    Row() {
      Text(title)
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
      Blank()
      if (detail.length > 0) {
        Text(detail)
          .fontSize(12)
          .fontColor(SUBTLE)
      }
    }
    .width('100%')
    .padding({ left: 4, right: 4 })
  }

  @Builder
  CurrentModelRow() {
    Row({ space: 12 }) {
      SymbolGlyph($r('sys.symbol.checkmark_circle_fill'))
        .fontSize(23)
        .fontColor([this.currentModel() ? INK : MUTED])
        .width(28)
        .height(28)
      Column({ space: 3 }) {
        Text(this.currentModelLabel())
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        if (this.currentModel()) {
          Text(this.modelSourceLabel(this.currentModel()))
            .fontSize(12)
            .fontColor(MUTED)
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .height(68)
    .padding({ left: 16, right: 16 })
    .backgroundColor(SOFT)
    .borderRadius(8)
  }

  @Builder
  AccountModelsSummaryRow() {
    Row({ space: 12 }) {
      SymbolGlyph($r('sys.symbol.cloud'))
        .fontSize(21)
        .fontColor([this.accountModels().length > 0 ? MUTED : SUBTLE])
        .width(28)
        .height(28)
      Column({ space: 3 }) {
        Text(RemoteI18n.t('settings.modelService.accountModelSummary'))
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(this.accountModels().length > 0 ?
          RemoteI18n.f('settings.modelService.syncedCount', String(this.accountModels().length)) :
          RemoteI18n.t('settings.modelService.accountEmpty'))
          .fontSize(12)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      if (this.isAccountModelSelected()) {
        SymbolGlyph($r('sys.symbol.checkmark_circle_fill'))
          .fontSize(18)
          .fontColor([INK])
      }
    }
    .width('100%')
    .height(62)
    .padding({ left: 16, right: 16 })
    .backgroundColor(SOFT)
    .borderRadius(8)
  }

  @Builder
  LocalModelRow() {
    Row({ space: 12 }) {
      SymbolGlyph($r('sys.symbol.wrench_and_screwdriver'))
        .fontSize(21)
        .fontColor([MUTED])
        .width(28)
        .height(28)
      Column({ space: 3 }) {
        Text(this.hasCompleteLocalModel() ? this.modelName : RemoteI18n.t('settings.modelService.notConfigured'))
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        if (this.hasCompleteLocalModel()) {
          Text(RemoteI18n.t('settings.modelService.localSource'))
            .fontSize(12)
            .fontColor(MUTED)
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      if (this.isLocalModelSelected()) {
        SymbolGlyph($r('sys.symbol.checkmark_circle_fill'))
          .fontSize(18)
          .fontColor([INK])
      }
      SymbolGlyph($r('sys.symbol.chevron_right'))
        .fontSize(14)
        .fontColor([MUTED])
    }
    .width('100%')
    .height(62)
    .padding({ left: 16, right: 16 })
    .backgroundColor(SOFT)
    .borderRadius(8)
    .onClick(() => {
      this.showLocalEditor = true;
      this.resetEditorDraft();
    })
  }

  @Builder
  ConfigField(label: string, placeholder: string, kind: string) {
    Column({ space: 8 }) {
      Text(label)
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
      TextInput({
        placeholder,
        text: kind === 'url' ? this.draftApiUrl : this.draftModelName
      })
        .width('100%')
        .height(52)
        .fontSize(15)
        .fontColor(INK)
        .placeholderColor(SUBTLE)
        .backgroundColor(SOFT)
        .borderRadius(8)
        .padding({ left: 14, right: 14 })
        .onFocus(() => {
          this.scrollFocusedFieldIntoView(kind);
        })
        .onBlur(() => {
          this.scheduleFocusedFieldReset();
        })
        .onChange((value: string) => {
          if (kind === 'url') {
            this.draftApiUrl = value;
          } else {
            this.draftModelName = value;
          }
          this.clearFeedback();
        })
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
  }

  @Builder
  ApiKeyField() {
    Column({ space: 8 }) {
      Row() {
        Text(RemoteI18n.t('settings.modelService.apiKey'))
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        if (this.hasApiKey && !this.clearApiKey) {
          Text(RemoteI18n.t('settings.modelService.keySaved'))
            .fontSize(12)
            .fontColor(MUTED)
        }
      }
      .width('100%')
      TextInput({
        placeholder: this.apiKeyPlaceholder(),
        text: this.draftApiKey
      })
        .type(InputType.Password)
        .showPasswordIcon(true)
        .width('100%')
        .height(52)
        .fontSize(15)
        .fontColor(INK)
        .placeholderColor(SUBTLE)
        .backgroundColor(SOFT)
        .borderRadius(8)
        .padding({ left: 14, right: 10 })
        .onFocus(() => {
          this.scrollFocusedFieldIntoView('key');
        })
        .onBlur(() => {
          this.scheduleFocusedFieldReset();
        })
        .onChange((value: string) => {
          this.draftApiKey = value;
          if (value.length > 0) {
            this.clearApiKey = false;
          }
          this.clearFeedback();
        })
      if (this.hasApiKey) {
        Text(this.clearApiKey ?
          RemoteI18n.t('settings.modelService.keepKey') :
          RemoteI18n.t('settings.modelService.clearKey'))
          .fontSize(13)
        .fontWeight(FontWeight.Medium)
        .fontColor(this.clearApiKey ? INK : RED)
        .width('100%')
        .onClick(() => {
          this.clearApiKey = !this.clearApiKey;
          this.draftApiKey = '';
          this.clearFeedback();
        })
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
  }

  private accountModels(): RemoteModelConfig[] {
    return this.modelCatalog.models.filter((model: RemoteModelConfig): boolean => {
      return model.enabled && model.id.startsWith('cloud:');
    });
  }

  private currentModel(): RemoteModelConfig | undefined {
    const candidates = [
      this.selectedModelId,
      this.modelCatalog.session_model_id || '',
      this.modelCatalog.default_models.primary || ''
    ];
    for (let index = 0; index < candidates.length; index += 1) {
      const modelId = candidates[index];
      const model = this.modelCatalog.models.find((item: RemoteModelConfig): boolean => {
        return item.id === modelId && item.enabled;
      });
      if (model) {
        return model;
      }
    }
    return undefined;
  }

  private currentModelLabel(): string {
    const model = this.currentModel();
    return model ? this.modelLabel(model) : RemoteI18n.t('settings.modelService.notConfigured');
  }

  private modelLabel(model: RemoteModelConfig): string {
    return model.model_name || model.name || model.id;
  }

  private modelSourceLabel(model?: RemoteModelConfig): string {
    if (!model) {
      return '';
    }
    return model.id === GENERAL_CHAT_LOCAL_MODEL_ID ?
      RemoteI18n.t('settings.modelService.localSource') :
      RemoteI18n.t('settings.modelService.accountSource');
  }

  private isAccountModelSelected(): boolean {
    const current = this.currentModel();
    return current !== undefined && current.id.startsWith('cloud:');
  }

  private isLocalModelSelected(): boolean {
    return this.currentModel()?.id === GENERAL_CHAT_LOCAL_MODEL_ID;
  }

  private hasCompleteLocalModel(): boolean {
    return this.apiUrl.trim().length > 0 && this.modelName.trim().length > 0 && this.hasApiKey;
  }

  private resetEditorDraft(): void {
    this.draftApiUrl = this.apiUrl;
    this.draftModelName = this.modelName;
    this.draftApiKey = '';
    this.clearApiKey = false;
    this.isTesting = false;
    this.clearFeedback();
    this.resetEditorScroll();
  }

  private resetEditorScroll(): void {
    this.clearFocusScrollTimer();
    this.clearBlurResetTimer();
    this.focusedFieldKind = '';
    this.contentScroller.scrollTo({ xOffset: 0, yOffset: 0, animation: false });
  }

  private apiKeyPlaceholder(): string {
    return this.hasApiKey && !this.clearApiKey ?
      RemoteI18n.t('settings.modelService.apiKeyKeepPlaceholder') :
      RemoteI18n.t('settings.modelService.apiKeyPlaceholder');
  }

  private async save(): Promise<void> {
    if (this.isSaving || this.isTesting) {
      return;
    }
    this.isSaving = true;
    this.clearFeedback();
    const error = await this.onSave(
      this.draftApiUrl,
      this.draftApiKey,
      this.draftModelName,
      this.clearApiKey
    );
    this.isSaving = false;
    if (error.length > 0) {
      this.feedbackText = error;
      this.feedbackIsError = true;
      return;
    }
    this.onSaved(
      this.draftApiUrl.trim(),
      this.draftModelName.trim(),
      !this.clearApiKey && (this.draftApiKey.trim().length > 0 || this.hasApiKey)
    );
    this.showLocalEditor = false;
    this.resetEditorScroll();
  }

  private async testConnection(): Promise<void> {
    if (this.isSaving || this.isTesting || !this.hasTestableApiKey()) {
      return;
    }
    this.isTesting = true;
    this.clearFeedback();
    const error = await this.onTest(
      this.draftApiUrl,
      this.draftApiKey,
      this.draftModelName,
      this.clearApiKey
    );
    this.isTesting = false;
    if (error.length > 0) {
      this.feedbackText = error;
      this.feedbackIsError = true;
      return;
    }
    this.feedbackText = RemoteI18n.t('settings.modelService.testSuccess');
    this.feedbackIsError = false;
  }

  private hasTestableApiKey(): boolean {
    return this.draftApiKey.trim().length > 0 || (this.hasApiKey && !this.clearApiKey);
  }

  private clearFeedback(): void {
    this.feedbackText = '';
    this.feedbackIsError = false;
  }

  private scrollFocusedFieldIntoView(kind: string): void {
    this.clearFocusScrollTimer();
    this.clearBlurResetTimer();
    this.focusedFieldKind = kind;
    const yOffset = kind === 'key' ? 118 : (kind === 'model' ? 244 : 0);
    this.focusScrollTimerId = setTimeout(() => {
      this.contentScroller.scrollTo({ xOffset: 0, yOffset, animation: true });
      this.focusScrollTimerId = 0;
    }, 280);
  }

  private scheduleFocusedFieldReset(): void {
    this.clearBlurResetTimer();
    this.blurResetTimerId = setTimeout(() => {
      this.focusedFieldKind = '';
      this.contentScroller.scrollTo({ xOffset: 0, yOffset: 0, animation: true });
      this.blurResetTimerId = 0;
    }, 180);
  }

  private clearFocusScrollTimer(): void {
    if (this.focusScrollTimerId !== 0) {
      clearTimeout(this.focusScrollTimerId);
      this.focusScrollTimerId = 0;
    }
  }

  private clearBlurResetTimer(): void {
    if (this.blurResetTimerId !== 0) {
      clearTimeout(this.blurResetTimerId);
      this.blurResetTimerId = 0;
    }
  }
}
