import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, MUTED } from './Theme';
import { ModelServiceSettingsPanel } from './ModelServiceSettingsPanel';

const SETTINGS_SHEET_BG: string = '#F4F4F7';
const SETTINGS_ROW_VALUE: string = '#8F8F94';

@Component
export struct SettingsSheet {
  @Prop generalChatApiUrl: string = '';
  @Prop generalChatModelName: string = '';
  @Prop hasGeneralChatApiKey: boolean = false;
  onClose: () => void = () => {};
  onSaveGeneralChatConfig: (
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
  @State showModelService: boolean = false;

  build() {
    Stack({ alignContent: Alignment.TopEnd }) {
      Scroll() {
        Column({ space: 0 }) {
          Text(RemoteI18n.t('settings.title'))
            .fontSize(28)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .width('100%')
            .margin({ bottom: 30 })

          this.SectionTitle(RemoteI18n.t('settings.modelService.section'))
          this.ModelServiceCard()

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
        Image($r('app.media.settings_close_x'))
          .width(28)
          .height(28)
          .objectFit(ImageFit.Contain)
      }
      .width(50)
      .height(50)
      .padding(0)
      .type(ButtonType.Circle)
      .backgroundColor(CARD)
      .shadow({ radius: 16, color: '#10000000', offsetY: 7 })
      .margin({ top: 22, right: 18 })
      .onClick(() => {
        this.onClose();
      })

      if (this.showModelService) {
        this.ModelServicePanel()
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(SETTINGS_SHEET_BG)
    .borderRadius({ topLeft: 34, topRight: 34 })
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
  ModelServiceCard() {
    Column() {
      this.SettingsRow(
        $r('app.media.settings_apps_grid'),
        RemoteI18n.t('settings.modelService.title'),
        this.modelServiceStatus(),
        true,
        () => {
          this.showModelService = true;
        }
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
        .fontColor(SETTINGS_ROW_VALUE)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(52)
    .padding({ left: 18, right: 18 })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  SettingsRow(icon: Resource, title: string, value: string, showChevron: boolean, action: () => void) {
    Row({ space: 16 }) {
      Image(icon)
        .width(23)
        .height(23)
        .objectFit(ImageFit.Contain)
      Text(title)
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Blank()
      Text(value)
        .fontSize(15)
        .fontColor(SETTINGS_ROW_VALUE)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .constraintSize({ maxWidth: 130 })
      if (showChevron) {
        Image($r('app.media.settings_chevron_right'))
          .width(10)
          .height(14)
          .objectFit(ImageFit.Contain)
      }
    }
    .width('100%')
    .height(52)
    .padding({ left: 18, right: 18 })
    .alignItems(VerticalAlign.Center)
    .onClick(action)
  }

  @Builder
  ModelServicePanel() {
    ModelServiceSettingsPanel({
      apiUrl: this.generalChatApiUrl,
      modelName: this.generalChatModelName,
      hasApiKey: this.hasGeneralChatApiKey,
      onClose: () => {
        this.showModelService = false;
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
    return this.generalChatApiUrl.length > 0 &&
      this.generalChatModelName.length > 0 && this.hasGeneralChatApiKey ?
      RemoteI18n.t('settings.modelService.configured') :
      RemoteI18n.t('settings.modelService.notConfigured');
  }
}
