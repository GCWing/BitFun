import { AppLocale } from '../../i18n/AppLocale';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, MODAL_SCRIM, SOFT } from './Theme';

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
    Stack({ alignContent: Alignment.Bottom }) {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor(MODAL_SCRIM)
        .onClick(() => {
          if (this.pendingLanguage.length === 0) {
            this.onClose();
          }
        })

      Column({ space: 0 }) {
        this.Header()
        this.LanguageList()
      }
      .width('100%')
      .constraintSize({ maxHeight: '70%' })
      .backgroundColor(CARD)
      .borderRadius({ topLeft: 28, topRight: 28 })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  Header() {
    Row() {
      Text(RemoteI18n.t('settings.language.choose'))
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
      .accessibilityText(RemoteI18n.t('common.close'))
      .onClick(() => {
        if (this.pendingLanguage.length === 0) {
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
  LanguageList() {
    Column() {
      ForEach(AppLocale.supported(), (language: string) => {
        this.LanguageRow(language)
      }, (language: string) => `${language}:${RemoteI18n.revision()}`)
    }
    .width('100%')
    .padding({ top: 8, bottom: 28 })
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
    .height(64)
    .padding({ left: 22, right: 22 })
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
