import { MobileDesignGeometry, MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { KeyboardAvoidMode } from '@kit.ArkUI';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteModelCatalog, RemoteModelConfig } from '../../model/RemoteModels';
import { GENERAL_CHAT_LOCAL_MODEL_ID } from '../../services/general-chat/GeneralChatConfigStore';
import { ModelServiceSettingsPolicy } from '../policy/ModelServiceSettingsPolicy';
import { SettingsSheetState } from '../state/SettingsSheetState';
import {
  CARD,
  INK,
  LINE,
  MUTED,
  SHADOW_FAINT,
  SOFT,
  STATUS_DANGER,
  STATUS_SUCCESS,
  SUBTLE,
  TRANSPARENT
} from './Theme';
import { SheetCloseHeader } from './SheetCloseHeader';
import { AppActionButton, AppActionStyle } from './AppActionButton';

@ComponentV2
export struct ModelServiceSettingsPanel {
  private readonly contentScroller: Scroller = new Scroller();
  private focusScrollTimerId: number = 0;
  private blurResetTimerId: number = 0;
  private previousKeyboardAvoidMode: KeyboardAvoidMode = KeyboardAvoidMode.OFFSET;
  @Param page: string = 'overview';
  @Param sheetState: SettingsSheetState = new SettingsSheetState();
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
  @Event onOpenAccountModels: () => void = () => {};
  @Event onOpenLocalEditor: () => void = () => {};
  @Event onSelectModel: (modelId: string) => void = (_modelId: string) => {};
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
  @Local isSaving: boolean = false;
  @Local isTesting: boolean = false;
  @Local feedbackText: string = '';
  @Local feedbackIsError: boolean = false;
  @Local focusedFieldKind: string = '';

  aboutToAppear(): void {
    if (this.sheetPage() === 'local') {
      this.previousKeyboardAvoidMode = this.getUIContext().getKeyboardAvoidMode();
      this.getUIContext().setKeyboardAvoidMode(KeyboardAvoidMode.RESIZE);
      this.resetEditorDraft();
    }
  }

  aboutToDisappear(): void {
    this.clearFocusScrollTimer();
    this.clearBlurResetTimer();
    if (this.sheetPage() === 'local') {
      this.getUIContext().setKeyboardAvoidMode(this.previousKeyboardAvoidMode);
    }
  }

  build() {
    Column({ space: 0 }) {
      this.Header()
      if (this.sheetPage() === 'local') {
        this.LocalEditor()
      } else if (this.sheetPage() === 'account') {
        this.AccountModelList()
      } else {
        this.ModelOverview()
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(CARD)
    .borderRadius({
      topLeft: MobileDesignGeometry.sheetTopRadius,
      topRight: MobileDesignGeometry.sheetTopRadius
    })
  }

  @Builder
  Header() {
    SheetCloseHeader({
      title: this.headerTitle(),
      showDivider: true,
      isEnabled: !this.isSaving && !this.isTesting,
      onClose: (): void => {
        if (!this.isSaving && !this.isTesting) {
          this.onClose();
        }
      }
    })
  }

  @Builder
  ModelOverview() {
    Column({ space: ModelServiceSettingsPolicy.OVERVIEW_SECTION_GAP }) {
      Column({ space: 8 }) {
        this.SectionHeader(RemoteI18n.t('settings.modelService.currentModel'))
        this.CurrentModelRow()
      }
      .width('100%')

      Column({ space: 8 }) {
        this.SectionHeader(RemoteI18n.t('settings.modelService.modelSources'))
        this.ModelSourceGroup()
      }
      .width('100%')
    }
    .width('100%')
    .padding({
      left: 16,
      right: 16,
      top: ModelServiceSettingsPolicy.OVERVIEW_TOP_PADDING,
      bottom: ModelServiceSettingsPolicy.OVERVIEW_BOTTOM_PADDING
    })
    .justifyContent(FlexAlign.Start)
  }

  @Builder
  ModelSourceGroup() {
    Column() {
      this.AccountModelsSummaryRow()
      Divider()
        .color(LINE)
        .strokeWidth(ModelServiceSettingsPolicy.SOURCE_DIVIDER_HEIGHT)
        .margin({ left: 56 })
      this.LocalModelRow()
    }
    .width('100%')
    .backgroundColor(SOFT)
    .borderRadius(8)
  }

  @Builder
  AccountModelList() {
    if (this.accountModels().length === 0) {
      Text(RemoteI18n.t('settings.modelService.accountEmpty'))
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontColor(MUTED)
        .width('100%')
        .height(ModelServiceSettingsPolicy.EMPTY_ACCOUNT_HEIGHT)
        .padding({
          left: 16,
          right: 16,
          top: ModelServiceSettingsPolicy.LIST_TOP_PADDING
        })
        .textAlign(TextAlign.Start)
    } else {
      List({ space: ModelServiceSettingsPolicy.ACCOUNT_MODEL_ROW_GAP }) {
        ForEach(this.accountModels(), (model: RemoteModelConfig) => {
          ListItem() {
            this.AccountModelRow(model)
          }
        }, (model: RemoteModelConfig) => model.id)
      }
      .width('100%')
      .height(ModelServiceSettingsPolicy.accountListContentHeight(this.accountModels().length))
      .padding({
        left: 10,
        right: 10,
        top: ModelServiceSettingsPolicy.LIST_TOP_PADDING,
        bottom: ModelServiceSettingsPolicy.LIST_BOTTOM_PADDING
      })
      .scrollBar(BarState.Auto)
      .edgeEffect(EdgeEffect.Spring)
      .divider(null)
    }
  }

  @Builder
  AccountModelRow(model: RemoteModelConfig) {
    Row({ space: 10 }) {
      Stack({ alignContent: Alignment.Center }) {
        if (this.isSelectedModel(model)) {
          SymbolGlyph($r('sys.symbol.checkmark_circle'))
            .fontSize(16)
            .fontColor([INK])
        }
      }
      .width(20)
      .height(20)
      Column({ space: 2 }) {
        Text(this.modelLabel(model))
          .fontSize(MobileDesignTypography.titleSmall.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(RemoteI18n.t('settings.modelService.accountSource'))
          .fontSize(MobileDesignTypography.labelSmall.size)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .height(ModelServiceSettingsPolicy.ACCOUNT_MODEL_ROW_HEIGHT)
    .padding({ left: 10, right: 10 })
    .backgroundColor(this.isSelectedModel(model) ? SOFT : TRANSPARENT)
    .borderRadius(9)
    .onClick(() => {
      this.onSelectModel(model.id);
    })
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
          AppActionButton({
            label: this.isTesting ? RemoteI18n.t('common.loading') :
              RemoteI18n.t('settings.modelService.testConnection'),
            style: AppActionStyle.Quiet,
            actionHeight: 50,
            isEnabled: !this.isSaving && !this.isTesting && this.hasTestableApiKey(),
            onAction: (): void => {
              this.testConnection();
            }
          })
            .layoutWeight(1)
          AppActionButton({
            label: this.isSaving ? RemoteI18n.t('common.loading') : RemoteI18n.t('common.save'),
            style: AppActionStyle.Primary,
            actionHeight: 50,
            isEnabled: !this.isSaving && !this.isTesting,
            onAction: (): void => {
              this.save();
            }
          })
            .layoutWeight(1)
        }
        .width('100%')

        if (!this.hasTestableApiKey()) {
          Text(RemoteI18n.t('settings.modelService.testNeedsKey'))
            .fontSize(MobileDesignTypography.labelSmall.size)
            .lineHeight(MobileDesignTypography.labelSmall.lineHeight)
            .fontColor(SUBTLE)
            .width('100%')
        }

        if (this.feedbackText.length > 0) {
          Text(this.feedbackText)
            .fontSize(MobileDesignTypography.bodySmall.size)
            .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
            .fontColor(this.feedbackIsError ? STATUS_DANGER : STATUS_SUCCESS)
            .width('100%')
        }
      }
      .width('100%')
      .padding({
        left: 16,
        right: 16,
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
  SectionHeader(title: string) {
    Text(title)
      .fontSize(MobileDesignTypography.bodyMedium.size)
      .fontWeight(FontWeight.Medium)
      .fontColor(MUTED)
      .width('100%')
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
          .fontSize(MobileDesignTypography.bodyLarge.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        if (this.currentModel()) {
          Text(this.modelSourceLabel(this.currentModel()))
            .fontSize(MobileDesignTypography.labelSmall.size)
            .fontColor(MUTED)
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .height(ModelServiceSettingsPolicy.CURRENT_ROW_HEIGHT)
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
          .fontSize(MobileDesignTypography.titleSmall.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(this.accountModels().length > 0 ?
          RemoteI18n.f('settings.modelService.syncedCount', String(this.accountModels().length)) :
          RemoteI18n.t('settings.modelService.accountEmpty'))
          .fontSize(MobileDesignTypography.labelSmall.size)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      SymbolGlyph($r('sys.symbol.chevron_right'))
        .fontSize(14)
        .fontColor([MUTED])
    }
    .width('100%')
    .height(ModelServiceSettingsPolicy.SOURCE_ROW_HEIGHT)
    .padding({ left: 16, right: 16 })
    .onClick(() => {
      this.onOpenAccountModels();
    })
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
          .fontSize(MobileDesignTypography.titleSmall.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        if (this.hasCompleteLocalModel()) {
          Text(RemoteI18n.t('settings.modelService.localSource'))
            .fontSize(MobileDesignTypography.labelSmall.size)
            .fontColor(MUTED)
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      .onClick(() => {
        if (this.localRowSelectsOnBodyTap()) {
          this.onSelectModel(GENERAL_CHAT_LOCAL_MODEL_ID);
          return;
        }
        this.onOpenLocalEditor();
      })
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.chevron_right'))
          .fontSize(14)
          .fontColor([MUTED])
      }
      .width(28)
      .height(28)
      .onClick(() => {
        this.onOpenLocalEditor();
      })
    }
    .width('100%')
    .height(ModelServiceSettingsPolicy.SOURCE_ROW_HEIGHT)
    .padding({ left: 16, right: 16 })
  }

  @Builder
  ConfigField(label: string, placeholder: string, kind: string) {
    Column({ space: 8 }) {
      Text(label)
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
      TextInput({
        placeholder,
        text: kind === 'url' ? this.sheetState.draftApiUrl : this.sheetState.draftModelName
      })
        .width('100%')
        .height(52)
        .fontSize(MobileDesignTypography.titleSmall.size)
        .fontColor(INK)
        .placeholderColor(SUBTLE)
        .backgroundColor(CARD)
        .border({ width: 0.5, color: LINE })
        .borderRadius(12)
        .shadow({ radius: 12, color: SHADOW_FAINT, offsetY: 4 })
        .padding({ left: 14, right: 14 })
        .onFocus(() => {
          this.scrollFocusedFieldIntoView(kind);
        })
        .onBlur(() => {
          this.scheduleFocusedFieldReset();
        })
        .onChange((value: string) => {
          if (kind === 'url') {
            this.sheetState.draftApiUrl = value;
          } else {
            this.sheetState.draftModelName = value;
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
          .fontSize(MobileDesignTypography.bodyMedium.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        if (this.hasApiKey && !this.sheetState.draftClearApiKey) {
          Text(RemoteI18n.t('settings.modelService.keySaved'))
            .fontSize(MobileDesignTypography.labelSmall.size)
            .fontColor(MUTED)
        }
      }
      .width('100%')
      TextInput({
        placeholder: this.apiKeyPlaceholder(),
        text: this.sheetState.draftApiKey
      })
        .type(InputType.Password)
        .showPasswordIcon(true)
        .width('100%')
        .height(52)
        .fontSize(MobileDesignTypography.titleSmall.size)
        .fontColor(INK)
        .placeholderColor(SUBTLE)
        .backgroundColor(CARD)
        .border({ width: 0.5, color: LINE })
        .borderRadius(12)
        .shadow({ radius: 12, color: SHADOW_FAINT, offsetY: 4 })
        .padding({ left: 14, right: 10 })
        .onFocus(() => {
          this.scrollFocusedFieldIntoView('key');
        })
        .onBlur(() => {
          this.scheduleFocusedFieldReset();
        })
        .onChange((value: string) => {
          this.sheetState.draftApiKey = value;
          if (value.length > 0) {
            this.sheetState.draftClearApiKey = false;
          }
          this.clearFeedback();
        })
      if (this.hasApiKey) {
        Text(this.sheetState.draftClearApiKey ?
          RemoteI18n.t('settings.modelService.keepKey') :
          RemoteI18n.t('settings.modelService.clearKey'))
          .fontSize(MobileDesignTypography.bodySmall.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(this.sheetState.draftClearApiKey ? INK : STATUS_DANGER)
          .width('100%')
          .onClick(() => {
            this.sheetState.draftClearApiKey = !this.sheetState.draftClearApiKey;
            this.sheetState.draftApiKey = '';
            this.clearFeedback();
          })
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
  }

  private sheetPage(): string {
    return ModelServiceSettingsPolicy.sheetPage(this.page);
  }

  private headerTitle(): string {
    const page = this.sheetPage();
    if (page === 'local') {
      return RemoteI18n.t('settings.modelService.localTitle');
    }
    if (page === 'account') {
      return RemoteI18n.t('settings.modelService.chooseAccountModel');
    }
    return RemoteI18n.t('settings.modelService.manageTitle');
  }

  private accountModels(): RemoteModelConfig[] {
    return ModelServiceSettingsPolicy.accountModels(this.modelCatalog);
  }

  private currentModel(): RemoteModelConfig | undefined {
    return ModelServiceSettingsPolicy.currentModel(this.modelCatalog, this.selectedModelId);
  }

  private currentModelLabel(): string {
    return ModelServiceSettingsPolicy.modelLabel(
      this.currentModel(),
      RemoteI18n.t('settings.modelService.notConfigured')
    );
  }

  private modelLabel(model: RemoteModelConfig): string {
    return ModelServiceSettingsPolicy.modelLabel(model, model.id);
  }

  private modelSourceLabel(model?: RemoteModelConfig): string {
    if (!model) {
      return '';
    }
    return ModelServiceSettingsPolicy.isLocalModel(model) ?
      RemoteI18n.t('settings.modelService.localSource') :
      RemoteI18n.t('settings.modelService.accountSource');
  }

  private isSelectedModel(model: RemoteModelConfig): boolean {
    const current = this.currentModel();
    return current !== undefined && current.id === model.id;
  }

  private hasCompleteLocalModel(): boolean {
    return ModelServiceSettingsPolicy.hasCompleteLocalModel(this.apiUrl, this.modelName, this.hasApiKey);
  }

  private localRowSelectsOnBodyTap(): boolean {
    return ModelServiceSettingsPolicy.localRowSelectsOnBodyTap(this.apiUrl, this.modelName, this.hasApiKey);
  }

  private resetEditorDraft(): void {
    this.sheetState.hydrateEditorDraft(this.apiUrl, this.modelName);
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
    return this.hasApiKey && !this.sheetState.draftClearApiKey ?
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
      this.sheetState.draftApiUrl,
      this.sheetState.draftApiKey,
      this.sheetState.draftModelName,
      this.sheetState.draftClearApiKey
    );
    this.isSaving = false;
    if (error.length > 0) {
      this.feedbackText = error;
      this.feedbackIsError = true;
      return;
    }
    this.onSaved(
      this.sheetState.draftApiUrl.trim(),
      this.sheetState.draftModelName.trim(),
      !this.sheetState.draftClearApiKey &&
        (this.sheetState.draftApiKey.trim().length > 0 || this.hasApiKey)
    );
    this.onClose();
  }

  private async testConnection(): Promise<void> {
    if (this.isSaving || this.isTesting || !this.hasTestableApiKey()) {
      return;
    }
    this.isTesting = true;
    this.clearFeedback();
    const error = await this.onTest(
      this.sheetState.draftApiUrl,
      this.sheetState.draftApiKey,
      this.sheetState.draftModelName,
      this.sheetState.draftClearApiKey
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
    return this.sheetState.draftApiKey.trim().length > 0 ||
      (this.hasApiKey && !this.sheetState.draftClearApiKey);
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
