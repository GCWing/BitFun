import { RemoteI18n } from '../../i18n/RemoteI18n';
import { DEFAULT_CLOUD_RELAY_URL } from '../../services/CloudAccountClient';
import { CARD, INK, MUTED, PRIMARY_ACTION, PRIMARY_ACTION_TEXT, RED, SUBTLE } from './Theme';
import { TemplateIcon } from './TemplateIcon';

@ComponentV2
export struct BitFunAccountLoginPage {
  @Event cloudLogin: (relayUrl: string, username: string, password: string) => Promise<string> =
    async (_relayUrl: string, _username: string, _password: string): Promise<string> => '';
  @Event onBack: () => void = () => {};
  @Event onLoginSuccess: () => void = () => {};
  @Local relayUrl: string = DEFAULT_CLOUD_RELAY_URL;
  @Local username: string = '';
  @Local password: string = '';
  @Local errorText: string = '';
  @Local isBusy: boolean = false;
  @Local showAdvanced: boolean = false;

  build() {
    Stack({ alignContent: Alignment.TopStart }) {
      Scroll() {
        Column({ space: 0 }) {
          Text(RemoteI18n.t('remote.settings.accountLoginTitle'))
            .fontSize(32)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .width('100%')
            .margin({ top: 0 })

          Text(RemoteI18n.t('remote.settings.accountLoginBody'))
            .fontSize(15)
            .lineHeight(22)
            .fontColor(MUTED)
            .width('100%')
            .margin({ top: 12, bottom: 42 })

          TextInput({ placeholder: RemoteI18n.t('connect.accountUsernamePlaceholder'), text: this.username })
            .height(58)
            .fontSize(17)
            .fontColor(INK)
            .placeholderColor(SUBTLE)
            .backgroundColor(CARD)
            .borderRadius(18)
            .padding({ left: 20, right: 20 })
            .onChange((value: string) => { this.username = value; })

          TextInput({ placeholder: RemoteI18n.t('connect.accountPasswordPlaceholder'), text: this.password })
            .height(58)
            .fontSize(17)
            .fontColor(INK)
            .placeholderColor(SUBTLE)
            .backgroundColor(CARD)
            .borderRadius(18)
            .padding({ left: 20, right: 20 })
            .margin({ top: 14 })
            .type(InputType.Password)
            .onChange((value: string) => { this.password = value; })

          Row() {
            Text(RemoteI18n.t('sheet.advancedOptions'))
              .fontSize(13)
              .fontColor(MUTED)
            Blank()
            if (this.showAdvanced) {
              SymbolGlyph($r('sys.symbol.chevron_up'))
                .fontSize(12)
                .fontColor([MUTED])
            } else {
              SymbolGlyph($r('sys.symbol.chevron_down'))
                .fontSize(12)
                .fontColor([MUTED])
            }
          }
          .width('100%')
          .height(44)
          .padding({ left: 4, right: 4 })
          .margin({ top: 12 })
          .alignItems(VerticalAlign.Center)
          .onClick(() => {
            this.showAdvanced = !this.showAdvanced;
          })

          if (this.showAdvanced) {
            Text(RemoteI18n.t('remote.settings.loginServer'))
              .fontSize(13)
              .fontColor(MUTED)
              .width('100%')
              .margin({ left: 4, top: 8, bottom: 8 })

            TextInput({ placeholder: RemoteI18n.t('remote.settings.relayUrlPlaceholder'), text: this.relayUrl })
              .height(52)
              .fontSize(14)
              .fontColor(INK)
              .placeholderColor(SUBTLE)
              .backgroundColor(CARD)
              .borderRadius(16)
              .padding({ left: 18, right: 18 })
              .onChange((value: string) => { this.relayUrl = value; })
          }

          if (this.errorText.length > 0) {
            Text(this.errorText)
              .fontSize(13)
              .lineHeight(19)
              .fontColor(RED)
              .width('100%')
              .margin({ top: 12 })
          }

          Button(this.isBusy ? RemoteI18n.t('remote.settings.accountSigningIn') :
            RemoteI18n.t('remote.settings.accountSignIn'))
            .height(56)
            .width('100%')
            .fontSize(17)
            .fontWeight(FontWeight.Bold)
            .fontColor(PRIMARY_ACTION_TEXT)
            .backgroundColor(PRIMARY_ACTION)
            .borderRadius(18)
            .margin({ top: this.errorText.length > 0 ? 22 : 30 })
            .opacity(this.canSubmit() ? 1 : 0.28)
            .enabled(this.canSubmit())
            .onClick(async () => {
              await this.submit();
            })
        }
        .width('100%')
        .padding({ left: 28, right: 28, top: 24, bottom: 44 })
        .alignItems(HorizontalAlign.Start)
      }
      .width('100%')
      .height('100%')
      .scrollBar(BarState.Off)

      Button() {
        TemplateIcon({
          src: $r('app.media.remote_ref_back'),
          iconWidth: 15,
          iconHeight: 23
        })
      }
      .width(44)
      .height(44)
      .padding(0)
      .type(ButtonType.Circle)
      .backgroundColor('#00000000')
      .position({ x: 28, y: 22 })
      .accessibilityText(RemoteI18n.t('common.back'))
      .onClick(() => {
        this.onBack();
      })
    }
    .width('100%')
    .height('100%')
  }

  private canSubmit(): boolean {
    return !this.isBusy && this.relayUrl.trim().length > 0 &&
      this.username.trim().length > 0 && this.password.length > 0;
  }

  private async submit(): Promise<void> {
    if (!this.canSubmit()) return;
    this.isBusy = true;
    this.errorText = '';
    try {
      await this.cloudLogin(this.relayUrl, this.username, this.password);
      this.password = '';
      this.onLoginSuccess();
    } catch (err) {
      this.errorText = err instanceof Error ? err.message : RemoteI18n.t('remote.settings.accountLoginFailed');
    } finally {
      this.isBusy = false;
    }
  }
}
