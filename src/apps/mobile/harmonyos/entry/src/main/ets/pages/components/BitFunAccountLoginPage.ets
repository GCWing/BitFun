import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { DEFAULT_CLOUD_RELAY_URL } from '../../services/CloudAccountClient';
import {
  CARD,
  INK,
  LINE,
  MUTED,
  PAGE_BG_FADE,
  SHADOW_FAINT,
  STATUS_DANGER,
  SUBTLE
} from './Theme';
import { SheetActionFooter } from './SheetActionFooter';
import { SheetCloseHeader } from './SheetCloseHeader';
import {
  SHEET_CONTENT_BOTTOM_PADDING,
  SHEET_CONTENT_MAX_WIDTH,
  SHEET_HORIZONTAL_PADDING
} from './SheetLayout';

const CREDENTIAL_ROW_HEIGHT: number = 60;

@ComponentV2
export struct BitFunAccountLoginPage {
  @Event cloudLogin: (relayUrl: string, username: string, password: string) => Promise<string> =
    async (_relayUrl: string, _username: string, _password: string): Promise<string> => '';
  @Event onClose: () => void = () => {};
  @Event onLoginSuccess: () => void = () => {};
  @Local relayUrl: string = DEFAULT_CLOUD_RELAY_URL;
  @Local username: string = '';
  @Local password: string = '';
  @Local errorText: string = '';
  @Local isBusy: boolean = false;
  @Local showAdvanced: boolean = false;

  build() {
    Column({ space: 0 }) {
      SheetCloseHeader({ onClose: this.onClose })

      Scroll() {
        Column() {
          this.LoginForm()
        }
        .width('100%')
        .padding({
          left: SHEET_HORIZONTAL_PADDING,
          right: SHEET_HORIZONTAL_PADDING,
          top: 0,
          bottom: SHEET_CONTENT_BOTTOM_PADDING
        })
        .alignItems(HorizontalAlign.Center)
      }
      .width('100%')
      .layoutWeight(1)
      .scrollBar(BarState.Off)

      SheetActionFooter({
        label: this.isBusy ? RemoteI18n.t('remote.settings.accountSigningIn') :
          RemoteI18n.t('remote.settings.accountSignIn'),
        primary: true,
        isEnabled: this.canSubmit(),
        onAction: (): void => {
          void this.submit();
        }
      })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  private LoginForm() {
    Column({ space: 0 }) {
      Text(RemoteI18n.t('remote.settings.accountLoginTitle'))
        .fontSize(MobileDesignTypography.displayMedium.size)
        .lineHeight(MobileDesignTypography.displayMedium.lineHeight)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .width('100%')
        .textAlign(TextAlign.Center)

      Text(RemoteI18n.t('remote.settings.accountLoginBody'))
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .lineHeight(MobileDesignTypography.bodyMedium.lineHeight)
        .fontColor(MUTED)
        .width('100%')
        .textAlign(TextAlign.Center)
        .margin({ top: 8, bottom: 24 })

      this.CredentialCard()
      this.AdvancedCard()

      if (this.errorText.length > 0) {
        Text(this.errorText)
          .fontSize(MobileDesignTypography.bodySmall.size)
          .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
          .fontColor(STATUS_DANGER)
          .width('100%')
          .margin({ top: 12, left: 4, right: 4 })
      }
    }
    .width('100%')
    .constraintSize({ maxWidth: SHEET_CONTENT_MAX_WIDTH })
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  private CredentialCard() {
    Column() {
      Row({ space: 12 }) {
        SymbolGlyph($r('sys.symbol.person'))
          .fontSize(21)
          .fontColor([MUTED])
          .width(24)
          .height(24)
        TextInput({
          placeholder: RemoteI18n.t('connect.accountUsernamePlaceholder'),
          text: this.username
        })
          .height(CREDENTIAL_ROW_HEIGHT)
          .layoutWeight(1)
          .fontSize(MobileDesignTypography.bodyLarge.size)
          .fontColor(INK)
          .placeholderColor(SUBTLE)
          .backgroundColor(PAGE_BG_FADE)
          .padding({ left: 0, right: 4 })
          .onChange((value: string) => { this.username = value; })
      }
      .width('100%')
      .height(CREDENTIAL_ROW_HEIGHT)
      .padding({ left: 18, right: 12 })
      .alignItems(VerticalAlign.Center)

      Divider()
        .strokeWidth(1)
        .color(LINE)
        .margin({ left: 54, right: 16 })

      Row({ space: 12 }) {
        SymbolGlyph($r('sys.symbol.lock'))
          .fontSize(20)
          .fontColor([MUTED])
          .width(24)
          .height(24)
        TextInput({
          placeholder: RemoteI18n.t('connect.accountPasswordPlaceholder'),
          text: this.password
        })
          .height(CREDENTIAL_ROW_HEIGHT)
          .layoutWeight(1)
          .fontSize(MobileDesignTypography.bodyLarge.size)
          .fontColor(INK)
          .placeholderColor(SUBTLE)
          .backgroundColor(PAGE_BG_FADE)
          .padding({ left: 0, right: 4 })
          .type(InputType.Password)
          .showPasswordIcon(true)
          .onChange((value: string) => { this.password = value; })
      }
      .width('100%')
      .height(CREDENTIAL_ROW_HEIGHT)
      .padding({ left: 18, right: 12 })
      .alignItems(VerticalAlign.Center)
    }
    .width('100%')
    .backgroundColor(CARD)
    .border({ width: 1, color: LINE })
    .borderRadius(24)
    .shadow({ radius: 16, color: SHADOW_FAINT, offsetY: 5 })
    .clip(true)
  }

  @Builder
  private AdvancedCard() {
    Column() {
      Row({ space: 14 }) {
        SymbolGlyph($r('sys.symbol.gearshape'))
          .fontSize(21)
          .fontColor([MUTED])
          .width(24)
          .height(24)
        Text(RemoteI18n.t('sheet.advancedOptions'))
          .fontSize(MobileDesignTypography.titleSmall.size)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .layoutWeight(1)
        SymbolGlyph(this.showAdvanced ?
          $r('sys.symbol.chevron_up') : $r('sys.symbol.chevron_down'))
          .fontSize(13)
          .fontColor([MUTED])
      }
      .width('100%')
      .height(58)
      .padding({ left: 18, right: 18 })
      .alignItems(VerticalAlign.Center)
      .onClick(() => {
        this.showAdvanced = !this.showAdvanced;
      })

      if (this.showAdvanced) {
        Divider()
          .strokeWidth(1)
          .color(LINE)
          .margin({ left: 18, right: 18 })

        Column({ space: 8 }) {
          Text(RemoteI18n.t('remote.settings.loginServer'))
            .fontSize(MobileDesignTypography.labelSmall.size)
            .fontColor(MUTED)
            .width('100%')

          TextInput({
            placeholder: RemoteI18n.t('remote.settings.relayUrlPlaceholder'),
            text: this.relayUrl
          })
            .height(48)
            .fontSize(MobileDesignTypography.bodyMedium.size)
            .fontColor(INK)
            .placeholderColor(SUBTLE)
            .backgroundColor(PAGE_BG_FADE)
            .border({ width: 1, color: LINE })
            .borderRadius(14)
            .padding({ left: 14, right: 14 })
            .onChange((value: string) => { this.relayUrl = value; })
        }
        .width('100%')
        .padding({ left: 18, right: 18, top: 14, bottom: 18 })
        .alignItems(HorizontalAlign.Start)
      }
    }
    .width('100%')
    .backgroundColor(CARD)
    .border({ width: 1, color: LINE })
    .borderRadius(20)
    .clip(true)
    .margin({ top: 14 })
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
