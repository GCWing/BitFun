import { MobileDesignGeometry, MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteModelCatalog, RemoteModelConfig } from '../../model/RemoteModels';
import { ModelServiceSettingsPolicy } from '../policy/ModelServiceSettingsPolicy';
import { SettingsSheetState } from '../state/SettingsSheetState';
import { CARD, INK, LINE, MUTED, PAGE_BG, SOFT, STATUS_DANGER } from './Theme';
import { AccountProfilePanel } from './AccountProfilePanel';
import { LanguageSettingsPanel } from './LanguageSettingsPanel';
import { ModelServiceSettingsPanel } from './ModelServiceSettingsPanel';
import { SheetCloseHeader } from './SheetCloseHeader';
import { SHEET_HORIZONTAL_PADDING, SHEET_TOP_RADIUS } from './SheetLayout';
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
  @Param fallbackUserId: string = '';
  @Param desktopName: string = '';
  @Param desktopId: string = '';
  @Param deviceId: string = '';
  @Param controlTargetType: string = 'none';
  @Param controlTargetDeviceId: string = '';
  @Param connectionState: string = 'idle';
  @Event onClose: () => void = () => {};
  @Event onOpenAccount: () => void = () => {};
  @Event onDisconnect: () => void = () => {};
  @Event onClearPairing: () => void = () => {};
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
    .borderRadius({ topLeft: SHEET_TOP_RADIUS, topRight: SHEET_TOP_RADIUS })
  }

  @Builder
  SettingsHome() {
    Column() {
      SheetCloseHeader({
        title: RemoteI18n.t('settings.title'),
        showDivider: true,
        onClose: this.onClose
      })

      Scroll() {
        Column({ space: 0 }) {
          AccountProfilePanel({
            accountUsername: this.accountUsername,
            accountUserId: this.authenticatedUserId,
            fallbackUserId: this.fallbackUserId,
            deviceId: this.deviceId,
            controlTargetType: this.controlTargetType,
            controlTargetDeviceId: this.controlTargetDeviceId,
            connectionState: this.connectionState,
            cloudLogout: this.cloudLogout,
            cloudListDevices: this.cloudListDevices,
            cloudSelectDevice: this.cloudSelectDevice,
            onReconnect: this.onReconnect,
            onOpenAccount: this.onOpenAccount,
            onLoggedOut: this.onClose,
            embeddedInSettings: true,
            connection: () => {
              this.CurrentConnectionSection()
            },
            preferences: () => {
              this.GeneralSettingsSection()
            },
            about: () => {
              this.AboutSettingsSection()
            }
          })
        }
        .width('100%')
        .padding({
          left: SHEET_HORIZONTAL_PADDING,
          right: SHEET_HORIZONTAL_PADDING,
          top: 22,
          bottom: 34
        })
      }
      .width('100%')
      .layoutWeight(1)
      .scrollBar(BarState.Off)
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  private CurrentConnectionSection() {
    if (this.hasControlTarget()) {
      this.SectionTitle(RemoteI18n.t('remote.settings.currentControl'))
      Column() {
        Row({ space: 14 }) {
          Stack({ alignContent: Alignment.Center }) {
            SymbolGlyph($r('sys.symbol.desktop'))
              .fontSize(22)
              .fontColor([MUTED])
              .opacity(0.68)
          }
          .width(28)
          .height(28)

          Column({ space: 3 }) {
            Text(this.connectionTitle())
              .fontSize(MobileDesignTypography.bodyLarge.size)
              .fontWeight(FontWeight.Medium)
              .fontColor(INK)
              .width('100%')
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
            Text(this.connectionDetail())
              .fontSize(MobileDesignTypography.bodySmall.size)
              .fontColor(MUTED)
              .width('100%')
          }
          .layoutWeight(1)
          .alignItems(HorizontalAlign.Start)
        }
        .width('100%')
        .constraintSize({ minHeight: 68 })
        .padding({ left: 18, right: 18, top: 12, bottom: 12 })
        .alignItems(VerticalAlign.Center)

        Divider().strokeWidth(1).color(LINE).margin({ left: 18, right: 18 })

        Row({ space: 8 }) {
          Text(this.connectionSourceLabel())
            .fontSize(MobileDesignTypography.bodySmall.size)
            .fontColor(MUTED)
            .padding({ left: 10, right: 10, top: 5, bottom: 5 })
            .backgroundColor(SOFT)
            .borderRadius(12)
          Blank()
          if (!this.isConnectedOrConnecting()) {
            Text(RemoteI18n.t('remote.settings.reconnect'))
              .fontSize(MobileDesignTypography.bodyMedium.size)
              .fontWeight(FontWeight.Medium)
              .fontColor(INK)
              .padding({ left: 10, right: 10, top: 8, bottom: 8 })
              .onClick(this.onReconnect)
          }
          Text(this.controlTargetType === 'room' ?
            RemoteI18n.t('remote.settings.disconnectTemporary') :
            RemoteI18n.t('remote.settings.disconnect'))
            .fontSize(MobileDesignTypography.bodyMedium.size)
            .fontWeight(FontWeight.Medium)
            .fontColor(STATUS_DANGER)
            .padding({ left: 10, right: 10, top: 8, bottom: 8 })
            .onClick(() => {
              if (this.controlTargetType === 'room') {
                this.onClearPairing();
              } else {
                this.onDisconnect();
              }
            })
        }
        .width('100%')
        .constraintSize({ minHeight: 54 })
        .padding({ left: 18, right: 10 })
        .alignItems(VerticalAlign.Center)
      }
      .width('100%')
      .backgroundColor(CARD)
      .borderRadius(MobileDesignGeometry.settingsCardRadius)
      .margin({ bottom: 24 })
    }
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
      .fontSize(MobileDesignTypography.bodySmall.size)
      .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
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
    .borderRadius(MobileDesignGeometry.settingsCardRadius)
    .margin({ bottom: 24 })
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
    .borderRadius(MobileDesignGeometry.settingsCardRadius)
    .margin({ bottom: 24 })
  }

  @Builder
  AboutCard() {
    Column() {
      this.StaticRow(RemoteI18n.t('settings.about.openbitfun'), 'HarmonyOS')
      Text('')
        .width('84%')
        .height(1)
        .backgroundColor(LINE)
      this.StaticRow(RemoteI18n.t('settings.about.version'), '1.0.0')
    }
    .width('100%')
    .backgroundColor(CARD)
    .borderRadius(MobileDesignGeometry.settingsCardRadius)
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
    .constraintSize({ minHeight: 56 })
    .padding({ left: 18, right: 18, top: 12, bottom: 12 })
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
    .constraintSize({ minHeight: 56 })
    .padding({ left: 18, right: 18, top: 12, bottom: 12 })
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

  private hasControlTarget(): boolean {
    return this.controlTargetType === 'room' || this.controlTargetType === 'account_device';
  }

  private isConnectedOrConnecting(): boolean {
    return this.connectionState === 'connected' || this.connectionState === 'reconnecting' ||
      this.connectionState === 'pairing' || this.connectionState === 'parsing';
  }

  private connectionTitle(): string {
    return this.desktopName || this.desktopId || RemoteI18n.t('remote.settings.noDesktop');
  }

  private connectionDetail(): string {
    if (this.connectionState === 'connected') {
      return RemoteI18n.t('connection.connected');
    }
    if (this.connectionState === 'reconnecting') {
      return RemoteI18n.t('connection.reconnecting');
    }
    return RemoteI18n.t('connection.disconnected');
  }

  private connectionSourceLabel(): string {
    return this.controlTargetType === 'room' ? RemoteI18n.t('remote.settings.qrPairing') :
      RemoteI18n.t('remote.settings.accountDevice');
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
