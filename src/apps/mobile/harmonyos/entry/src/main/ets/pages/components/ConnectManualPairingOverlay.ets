import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, MUTED, SCRIM, SHADOW_FAINT } from './Theme';
import { AppActionButton, AppActionStyle } from './AppActionButton';

@ComponentV2
export struct ConnectManualPairingOverlay {
  @Param remoteUrl: string = '';
  @Param userIdInput: string = '';
  @Param password: string = '';
  @Param requiresAccountAuth: boolean = false;
  @Param canSubmit: boolean = false;
  @Event onRemoteUrlChange: (value: string) => void = (_value: string) => {};
  @Event onUserIdChange: (value: string) => void = (_value: string) => {};
  @Event onPasswordChange: (value: string) => void = (_value: string) => {};
  @Event onCancel: () => void = () => {};
  @Event onSubmit: () => void = () => {};

  build() {
    Stack() {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor(SCRIM)
        .onClick(this.onCancel)

      Column({ space: 20 }) {
        Text(this.requiresAccountAuth ?
          RemoteI18n.t('connect.accountPairTitle') : RemoteI18n.t('connect.manualPair'))
          .fontSize(MobileDesignTypography.displayLarge.size)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
        Text(this.requiresAccountAuth ?
          RemoteI18n.t('connect.accountPairIntro') : RemoteI18n.t('connect.manualPairBody'))
          .fontSize(MobileDesignTypography.titleMedium.size)
          .lineHeight(MobileDesignTypography.titleMedium.lineHeight)
          .fontColor(MUTED)
          .width('100%')
        TextInput({ placeholder: RemoteI18n.t('connect.pairCodePlaceholder'), text: this.remoteUrl })
          .height(62)
          .fontSize(MobileDesignTypography.bodyLarge.size)
          .fontColor(INK)
          .backgroundColor(CARD)
          .border({ width: 0.5, color: LINE })
          .borderRadius(31)
          .shadow({ radius: 14, color: SHADOW_FAINT, offsetY: 5 })
          .padding({ left: 20, right: 20 })
          .defaultFocus(true)
          .onChange(this.onRemoteUrlChange)
        if (this.requiresAccountAuth) {
          TextInput({
            placeholder: RemoteI18n.t('connect.accountUsernamePlaceholder'),
            text: this.userIdInput
          })
            .height(56)
            .fontSize(MobileDesignTypography.bodyLarge.size)
            .fontColor(INK)
            .backgroundColor(CARD)
            .border({ width: 0.5, color: LINE })
            .borderRadius(28)
            .shadow({ radius: 14, color: SHADOW_FAINT, offsetY: 5 })
            .padding({ left: 20, right: 20 })
            .onChange(this.onUserIdChange)
          TextInput({ placeholder: RemoteI18n.t('connect.accountPasswordPlaceholder'), text: this.password })
            .height(56)
            .fontSize(MobileDesignTypography.bodyLarge.size)
            .fontColor(INK)
            .backgroundColor(CARD)
            .border({ width: 0.5, color: LINE })
            .borderRadius(28)
            .shadow({ radius: 14, color: SHADOW_FAINT, offsetY: 5 })
            .padding({ left: 20, right: 20 })
            .type(InputType.Password)
            .onChange(this.onPasswordChange)
          Text(RemoteI18n.t('connect.accountPairBody'))
            .fontSize(MobileDesignTypography.bodySmall.size)
            .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
            .fontColor(MUTED)
            .width('100%')
        }
        Row({ space: 12 }) {
          AppActionButton({
            label: RemoteI18n.t('common.cancel'),
            style: AppActionStyle.Quiet,
            actionHeight: 58,
            onAction: this.onCancel
          })
            .layoutWeight(1)
          AppActionButton({
            label: RemoteI18n.t('connect.pair'),
            style: AppActionStyle.Primary,
            actionHeight: 58,
            isEnabled: this.canSubmit,
            onAction: this.onSubmit
          })
            .layoutWeight(1)
        }
        .width('100%')
      }
      .width('82%')
      .constraintSize({ maxWidth: 520 })
      .padding({ left: 28, right: 28, top: 30, bottom: 28 })
      .backgroundColor(CARD)
      .borderRadius(34)
      .border({ width: 1, color: LINE })
    }
    .width('100%')
    .height('100%')
  }
}
