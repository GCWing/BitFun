import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteModelCatalog, RemoteModelConfig } from '../../model/RemoteModels';
import { ModelServiceSettingsPolicy } from '../policy/ModelServiceSettingsPolicy';
import { SettingsSheetState } from '../state/SettingsSheetState';
import { CARD, INK, LINE, MUTED, PAGE_BG } from './Theme';
import { AccountProfilePanel } from './AccountProfilePanel';
import { LanguageSettingsPanel } from './LanguageSettingsPanel';
import { ModelServiceSettingsPanel } from './ModelServiceSettingsPanel';
import { CloudAccountDevice } from '../../services/CloudAccountClient';

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
  @Param controlTargetDeviceId: string = '';
  @Param connectionState: string = 'idle';
  @Event onClose: () => void = () => {};
  @Event onOpenAccount: () => void = () => {};
  @Event onReconnect: () => void = () => {};
  @Event cloudLogout: () => Promise<void> = async (): Promise<void> => {};
  @Event cloudListDevices: () => Promise<CloudAccountDevice[]> =
    async (): Promise<CloudAccountDevice[]> => [];
  @Event cloudSelectDevice: (device: CloudAccountDevice) => Promise<void> =
    async (_device: CloudAccountDevice): Promise<void> => {};
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
    Column() {
      Row() {
        Text(RemoteI18n.t('settings.title'))
          .fontSize(MobileDesignTypography.displayLarge.size)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
        Blank()
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
        .accessibilityText(RemoteI18n.t('common.close'))
        .onClick(() => {
          this.onClose();
        })
      }
      .width('100%')
      .height(84)
      .padding({ left: 28, right: 18, top: 10 })
      .alignItems(VerticalAlign.Center)
      .backgroundColor(PAGE_BG)

      Divider()
        .strokeWidth(1)
        .color(LINE)

      Scroll() {
        Column({ space: 0 }) {
          AccountProfilePanel({
            accountUsername: this.accountUsername,
            accountUserId: this.authenticatedUserId,
            deviceId: this.deviceId,
            controlTargetDeviceId: this.controlTargetDeviceId,
            connectionState: this.connectionState,
            cloudLogout: this.cloudLogout,
            cloudListDevices: this.cloudListDevices,
            cloudSelectDevice: this.cloudSelectDevice,
            onReconnect: this.onReconnect,
            onLoggedOut: this.onClose,
            embeddedInSettings: true,
            preferences: () => {
              this.GeneralSettingsSection()
            },
            about: () => {
              this.AboutSettingsSection()
            }
          })
        }
        .width('100%')
        .padding({ left: 24, right: 24, top: 22, bottom: 34 })
      }
      .width('100%')
      .layoutWeight(1)
      .scrollBar(BarState.Off)
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  private GeneralSettingsSection() {
    this.SectionTitle(RemoteI18n.t('settings.general.section'))
    this.GeneralCard()
    this.SectionTitle(RemoteI18n.t('settings.model.section'))
    this.ModelCard()
  }

  @Builder
  private AboutSettingsSection() {
    this.SectionTitle(RemoteI18n.t('settings.about.section'))
    this.AboutCard()
  }

  @Builder
  SectionTitle(title: string) {
    Text(title)
      .fontSize(MobileDesignTypography.titleSmall.size)
      .lineHeight(MobileDesignTypography.titleSmall.lineHeight)
      .fontWeight(FontWeight.Medium)
      .fontColor(MUTED)
      .width('100%')
      .margin({ left: 8, bottom: 10 })
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
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(20)
    .margin({ bottom: 28 })
  }

  @Builder
  ModelCard() {
    Column() {
      this.SettingsRow(
        RemoteI18n.t('settings.model.default'),
        this.modelServiceStatus(),
        true,
        () => {
          this.sheetState.setPage('overview');
        },
        $r('sys.symbol.square_grid_2x2')
      )
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(20)
    .margin({ bottom: 28 })
  }

  @Builder
  AboutCard() {
    Column() {
      this.StaticRow(RemoteI18n.t('settings.about.bitfun'), 'HarmonyOS')
      Text('')
        .width('84%')
        .height(1)
        .backgroundColor(LINE)
      this.StaticRow(RemoteI18n.t('settings.about.version'), '1.0.0')
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(20)
  }

  @Builder
  StaticRow(title: string, value: string) {
    Row({ space: 16 }) {
      Text(title)
        .fontSize(MobileDesignTypography.bodyLarge.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Text(value)
        .fontSize(MobileDesignTypography.titleSmall.size)
        .fontColor(MUTED)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(56)
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
        .fontSize(MobileDesignTypography.bodyLarge.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Text(value)
        .fontSize(MobileDesignTypography.titleSmall.size)
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
    .height(56)
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
