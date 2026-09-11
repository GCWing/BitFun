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
export struct OpenBitFunAccountLoginPage {
  @Event cloudLogin: () => Promise<string> =
    async (): Promise<string> => '';
  @Event cloudCancelLogin: () => void = () => {};
  @Event onClose: () => void = () => {};
  @Event onLoginSuccess: () => void = () => {};
  @Local errorText: string = '';
  @Local isBusy: boolean = false;

  aboutToDisappear(): void { this.cloudCancelLogin(); }

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

  private canSubmit(): boolean { return !this.isBusy; }

  private async submit(): Promise<void> {
    if (!this.canSubmit()) return;
    this.isBusy = true;
    this.errorText = '';
    try {
      await this.cloudLogin();
      this.onLoginSuccess();
    } catch (err) {
      this.errorText = err instanceof Error ? err.message : RemoteI18n.t('remote.settings.accountLoginFailed');
    } finally {
      this.isBusy = false;
    }
  }
}
