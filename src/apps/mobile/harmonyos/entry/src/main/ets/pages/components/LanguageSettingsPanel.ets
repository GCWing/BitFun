import { AppLocale } from '../../i18n/AppLocale';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ModelServiceSettingsPolicy } from '../policy/ModelServiceSettingsPolicy';
import { CARD, INK, LINE, MUTED } from './Theme';

@ComponentV2
export struct LanguageSettingsPanel {
  @Param selectedLanguage: string = AppLocale.DEFAULT;
  @Event onClose: () => void = () => {};
  @Event onSelect: (language: string) => Promise<void> = async (_language: string) => {};
  @Local pendingLanguage: string = '';

  aboutToAppear(): void {
    this.pendingLanguage = '';
  }

  build() {
    Column({ space: 0 }) {
      this.Header()
      this.LanguageList()
    }
    .width('100%')
    .height('100%')
    .backgroundColor(CARD)
    .borderRadius({ topLeft: 20, topRight: 20 })
  }

  @Builder
  Header() {
    Row() {
      Text(RemoteI18n.t('settings.language.choose'))
        .fontSize(18)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
      Blank()
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.xmark'))
          .fontSize(18)
          .fontColor([MUTED])
      }
      .width(32)
      .height(32)
      .accessibilityText(RemoteI18n.t('common.close'))
      .onClick(() => {
        if (this.pendingLanguage.length === 0) {
          this.onClose();
        }
      })
    }
    .width('100%')
    .height(ModelServiceSettingsPolicy.HEADER_HEIGHT)
    .padding({ left: 16, right: 16 })
    .border({ width: { bottom: 1 }, color: LINE })
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  LanguageList() {
    Column() {
      ForEach(AppLocale.supported(), (language: string) => {
        this.LanguageRow(language)
      }, (language: string) => `${language}:${RemoteI18n.revision()}`)
    }
    .width('100%')
    .padding({
      top: ModelServiceSettingsPolicy.LANGUAGE_LIST_TOP_PADDING,
      bottom: ModelServiceSettingsPolicy.LANGUAGE_LIST_BOTTOM_PADDING
    })
  }

  @Builder
  LanguageRow(language: string) {
    Row() {
      Text(RemoteI18n.translate(language, AppLocale.nativeNameKey(language)))
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .layoutWeight(1)
      if (language === this.selectedLanguage) {
        SymbolGlyph($r('sys.symbol.checkmark'))
          .fontSize(18)
          .fontColor([INK])
      }
    }
    .width('100%')
    .height(ModelServiceSettingsPolicy.LANGUAGE_ROW_HEIGHT)
    .padding({ left: 16, right: 16 })
    .alignItems(VerticalAlign.Center)
    .opacity(this.pendingLanguage.length > 0 && this.pendingLanguage !== language ? 0.45 : 1)
    .onClick(() => {
      void this.select(language);
    })
  }

  private async select(language: string): Promise<void> {
    if (language === this.selectedLanguage || this.pendingLanguage.length > 0) {
      return;
    }
    this.pendingLanguage = language;
    try {
      await this.onSelect(language);
      this.onClose();
    } finally {
      this.pendingLanguage = '';
    }
  }
}
