import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteModelCatalog, RemoteModelConfig } from '../../model/RemoteModels';
import { ModelServiceSettingsPolicy } from '../policy/ModelServiceSettingsPolicy';
import { SettingsSheetState } from '../state/SettingsSheetState';
import { CARD, INK, LINE, MUTED, PAGE_BG } from './Theme';
import { LanguageSettingsPanel } from './LanguageSettingsPanel';
import { ModelServiceSettingsPanel } from './ModelServiceSettingsPanel';
import { DefaultAccountAvatar } from './DefaultAccountAvatar';

@ComponentV2
export struct SettingsSheet {
  @Param sheetState: SettingsSheetState = new SettingsSheetState();
  @Param generalChatApiUrl: string = '';
  @Param generalChatModelName: string = '';
  @Param hasGeneralChatApiKey: boolean = false;
  @Param generalChatModelCatalog: RemoteModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @Param selectedGeneralChatModelId: string = '';
  @Param accountUsername: string = '';
  @Param authenticatedUserId: string = '';
  @Param deviceId: string = '';
  @Event onClose: () => void = () => {};
  @Event onOpenAccount: () => void = () => {};
  @Event onSetLanguage: (language: string) => Promise<void> = async (_language: string) => {};
  @Event onSelectGeneralChatModel: (modelId: string) => Promise<boolean> = async (_modelId: string) => false;
  @Event onSaveGeneralChatConfig: (
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
  @Event onTestGeneralChatConfig: (
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
  aboutToAppear(): void {
    this.syncGeneralChatConfig(this.generalChatApiUrl, this.generalChatModelName, this.hasGeneralChatApiKey);
  }

  build() {
    Stack({ alignContent: Alignment.TopEnd }) {
      if (this.sheetState.page === 'language') {
        this.LanguagePanel()
      } else if (this.sheetState.page === 'overview') {
        this.ModelOverviewSheet()
      } else if (this.sheetState.page === 'account') {
        this.AccountModelsSheet()
      } else if (this.sheetState.page === 'local') {
        this.LocalEditorSheet()
      } else {
        this.SettingsHome()
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
    .borderRadius({ topLeft: 34, topRight: 34 })
  }

  @Builder
  SettingsHome() {
    Stack({ alignContent: Alignment.TopEnd }) {
      Scroll() {
        Column({ space: 0 }) {
          Text(RemoteI18n.t('settings.title'))
            .fontSize(28)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .width('100%')
            .margin({ bottom: 30 })

          this.AccountEntry()

          this.SectionTitle(RemoteI18n.t('settings.general.section'))
          this.GeneralCard()

          this.SectionTitle(RemoteI18n.t('settings.about.section'))
          this.AboutCard()
        }
        .width('100%')
        .padding({ left: 16, right: 16, top: 64, bottom: 34 })
      }
      .width('100%')
      .height('100%')
      .scrollBar(BarState.Off)

      Button() {
        SymbolGlyph($r('sys.symbol.xmark'))
          .fontSize(18)
          .fontColor([INK])
      }
      .width(40)
      .height(40)
      .padding(0)
      .type(ButtonType.Circle)
      .backgroundColor(CARD)
      .margin({ top: 22, right: 18 })
      .accessibilityText(RemoteI18n.t('common.close'))
      .onClick(() => {
        this.onClose();
      })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  AccountEntry() {
    Row({ space: 12 }) {
      DefaultAccountAvatar({ avatarSize: 34 })
      Column({ space: 2 }) {
        Text(RemoteI18n.t('remote.settings.profile'))
          .fontSize(16).fontWeight(FontWeight.Medium).fontColor(INK)
        Text(this.accountUsername || (this.authenticatedUserId.length > 0 ?
          RemoteI18n.t('remote.settings.accountSignedIn') :
          RemoteI18n.t('remote.settings.accountNotSignedIn')))
          .fontSize(13).fontColor(MUTED)
          .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      }.layoutWeight(1).alignItems(HorizontalAlign.Start)
      SymbolGlyph($r('sys.symbol.chevron_right'))
        .fontSize(14).fontColor([MUTED]).opacity(0.72)
    }
    .width('100%').height(64).padding({ left: 18, right: 18 })
    .backgroundColor(CARD).borderRadius(8).margin({ bottom: 24 })
    .onClick(() => this.onOpenAccount())
  }

  @Builder
  SectionTitle(title: string) {
    Text(title)
      .fontSize(18)
      .fontWeight(FontWeight.Bold)
      .fontColor(MUTED)
      .width('100%')
      .margin({ left: 12, bottom: 8 })
  }

  @Builder
  GeneralCard() {
    Column() {
      this.SettingsRow(
        RemoteI18n.t('settings.language.title'),
        RemoteI18n.nativeName(),
        true,
        () => {
          this.sheetState.setPage('language');
        },
        $r('sys.symbol.textformat')
      )
      Text('')
        .width('84%')
        .height(1)
        .backgroundColor(LINE)
      this.SettingsRow(
        RemoteI18n.t('settings.modelService.title'),
        this.modelServiceStatus(),
        true,
        () => {
          this.sheetState.setPage('overview');
        },
        $r('sys.symbol.square_grid_2x2')
      )
    }
    .width('100%')
    .padding({ top: 5, bottom: 5 })
    .backgroundColor(CARD)
    .borderRadius(8)
    .margin({ bottom: 24 })
  }

  @Builder
  AboutCard() {
    Column() {
      this.StaticRow(RemoteI18n.t('settings.about.product'), 'BitFun HarmonyOS')
      Text('')
        .width('84%')
        .height(1)
        .backgroundColor(LINE)
      this.StaticRow(RemoteI18n.t('settings.about.version'), '1.0.0')
    }
    .width('100%')
    .padding({ top: 5, bottom: 5 })
    .backgroundColor(CARD)
    .borderRadius(8)
  }

  @Builder
  StaticRow(title: string, value: string) {
    Row({ space: 16 }) {
      Text(title)
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Text(value)
        .fontSize(15)
        .fontColor(MUTED)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(52)
    .padding({ left: 18, right: 18 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  SettingsRow(title: string, value: string, showChevron: boolean, action: () => void, icon: Resource) {
    Row({ space: 16 }) {
      SymbolGlyph(icon)
        .fontSize(20)
        .fontColor([MUTED])
        .width(23)
        .height(23)
      Text(title)
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Text(value)
        .fontSize(15)
        .fontColor(MUTED)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .constraintSize({ maxWidth: 130 })
      if (showChevron) {
        SymbolGlyph($r('sys.symbol.chevron_right'))
          .fontSize(14)
          .fontColor([MUTED])
      }
    }
    .width('100%')
    .height(52)
    .padding({ left: 18, right: 18 })
    .alignItems(VerticalAlign.Center)
    .onClick(action)
  }

  @Builder
  LanguagePanel() {
    LanguageSettingsPanel({
      selectedLanguage: RemoteI18n.language(),
      onClose: () => {
        this.sheetState.setPage(ModelServiceSettingsPolicy.parentSettingsPage('language'));
      },
      onSelect: (language: string) => {
        return this.onSetLanguage(language);
      }
    })
  }

  @Builder
  ModelOverviewSheet() {
    ModelServiceSettingsPanel({
      page: 'overview',
      sheetState: this.sheetState,
      apiUrl: this.sheetState.savedGeneralChatApiUrl,
      modelName: this.sheetState.savedGeneralChatModelName,
      hasApiKey: this.sheetState.savedGeneralChatHasApiKey,
      modelCatalog: this.generalChatModelCatalog,
      selectedModelId: this.selectedGeneralChatModelId,
      onClose: () => {
        this.sheetState.setPage(ModelServiceSettingsPolicy.parentSettingsPage('overview'));
      },
      onOpenAccountModels: () => {
        this.sheetState.setPage('account');
      },
      onOpenLocalEditor: () => {
        this.sheetState.setPage('local');
      },
      onSelectModel: (modelId: string) => {
        void this.selectGeneralChatModel(modelId);
      }
    })
  }

  @Builder
  AccountModelsSheet() {
    ModelServiceSettingsPanel({
      page: 'account',
      sheetState: this.sheetState,
      apiUrl: this.sheetState.savedGeneralChatApiUrl,
      modelName: this.sheetState.savedGeneralChatModelName,
      hasApiKey: this.sheetState.savedGeneralChatHasApiKey,
      modelCatalog: this.generalChatModelCatalog,
      selectedModelId: this.selectedGeneralChatModelId,
      onClose: () => {
        this.sheetState.setPage(ModelServiceSettingsPolicy.parentSettingsPage('account'));
      },
      onSelectModel: (modelId: string) => {
        void this.selectGeneralChatModel(modelId).then((selected: boolean) => {
          if (selected) {
            this.sheetState.setPage('overview');
          }
        });
      }
    })
  }

  @Builder
  LocalEditorSheet() {
    ModelServiceSettingsPanel({
      page: 'local',
      sheetState: this.sheetState,
      apiUrl: this.sheetState.savedGeneralChatApiUrl,
      modelName: this.sheetState.savedGeneralChatModelName,
      hasApiKey: this.sheetState.savedGeneralChatHasApiKey,
      modelCatalog: this.generalChatModelCatalog,
      selectedModelId: this.selectedGeneralChatModelId,
      onClose: () => {
        this.sheetState.setPage(ModelServiceSettingsPolicy.parentSettingsPage('local'));
      },
      onSaved: (
        apiUrl: string,
        modelName: string,
        hasApiKey: boolean
      ) => {
        this.syncGeneralChatConfig(apiUrl, modelName, hasApiKey);
      },
      onTest: (
        apiUrl: string,
        apiKey: string,
        modelName: string,
        clearApiKey: boolean
      ) => {
        return this.onTestGeneralChatConfig(apiUrl, apiKey, modelName, clearApiKey);
      },
      onSave: (
        apiUrl: string,
        apiKey: string,
        modelName: string,
        clearApiKey: boolean
      ) => {
        return this.onSaveGeneralChatConfig(apiUrl, apiKey, modelName, clearApiKey);
      }
    })
  }

  private modelServiceStatus(): string {
    const model = this.selectedGeneralChatModel();
    return ModelServiceSettingsPolicy.modelLabel(model, RemoteI18n.t('settings.modelService.notConfigured'));
  }

  private selectedGeneralChatModel(): RemoteModelConfig | undefined {
    return ModelServiceSettingsPolicy.currentModel(
      this.generalChatModelCatalog,
      this.selectedGeneralChatModelId
    );
  }

  private async selectGeneralChatModel(modelId: string): Promise<boolean> {
    return this.onSelectGeneralChatModel(modelId);
  }

  private syncGeneralChatConfig(apiUrl: string, modelName: string, hasApiKey: boolean): void {
    this.sheetState.syncSaved(apiUrl, modelName, hasApiKey);
  }
}
