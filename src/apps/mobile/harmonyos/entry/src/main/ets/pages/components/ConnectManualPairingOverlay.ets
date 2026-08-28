import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, MODAL_SCRIM, MUTED, PRIMARY_ACTION, PRIMARY_ACTION_TEXT, SOFT, SUBTLE } from './Theme';

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
        .backgroundColor(MODAL_SCRIM)
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
          .backgroundColor(SOFT)
          .borderRadius(31)
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
            .backgroundColor(SOFT)
            .borderRadius(28)
            .padding({ left: 20, right: 20 })
            .onChange(this.onUserIdChange)
          TextInput({ placeholder: RemoteI18n.t('connect.accountPasswordPlaceholder'), text: this.password })
            .height(56)
            .fontSize(MobileDesignTypography.bodyLarge.size)
            .fontColor(INK)
            .backgroundColor(SOFT)
            .borderRadius(28)
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
          Button(RemoteI18n.t('common.cancel'))
            .layoutWeight(1)
            .height(58)
            .fontSize(MobileDesignTypography.labelLarge.size)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .backgroundColor(SOFT)
            .borderRadius(29)
            .onClick(this.onCancel)
          Button(RemoteI18n.t('connect.pair'))
            .layoutWeight(1)
            .height(58)
            .fontSize(MobileDesignTypography.labelLarge.size)
            .fontWeight(FontWeight.Bold)
            .fontColor(this.canSubmit ? PRIMARY_ACTION_TEXT : SUBTLE)
            .backgroundColor(this.canSubmit ? PRIMARY_ACTION : SOFT)
            .borderRadius(29)
            .enabled(this.canSubmit)
            .onClick(this.onSubmit)
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
