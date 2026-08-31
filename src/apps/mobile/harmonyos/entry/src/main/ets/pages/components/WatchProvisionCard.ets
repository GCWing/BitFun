import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { WatchProvisionPhase, WatchProvisionState } from '../state/WatchProvisionState';
import { CARD, STATUS_SUCCESS, INK, LINE, SCRIM, MUTED, PRIMARY_ACTION, CONTENT_ON_ACTION, STATUS_DANGER, SOFT,
  SUBTLE } from './Theme';

/**
 * The one place a watch can be added to the account. It is deliberately modal:
 * granting a device a 30-day credential should interrupt, not sit in a corner
 * waiting to be noticed.
 */
@ComponentV2
export struct WatchProvisionCard {
  @Param state: WatchProvisionState = new WatchProvisionState();
  @Event onApprove: (password: string) => void = (_password: string) => {};
  @Event onReject: () => void = () => {};
  @Event onDismiss: () => void = () => {};
  @Local password: string = '';

  build() {
    if (this.state.visible()) {
      Stack() {
        Text('')
          .width('100%')
          .height('100%')
          .backgroundColor(SCRIM)
          .onClick(() => {
            // Tapping away while the desktop is minting would leave the watch
            // with no answer and no card to explain it.
            if (this.state.dismissible()) {
              this.onDismiss();
            }
          })

        Column({ space: 18 }) {
          Text(RemoteI18n.t('watchProvision.title'))
            .fontSize(MobileDesignTypography.headlineLarge.size)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
            .width('100%')

          Column({ space: 6 }) {
            Text(this.state.deviceName)
              .fontSize(MobileDesignTypography.headlineSmall.size)
              .fontWeight(FontWeight.Medium)
              .fontColor(INK)
              .width('100%')
              .maxLines(2)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
            Text(RemoteI18n.f('watchProvision.deviceId', this.state.deviceIdLabel))
              .fontSize(MobileDesignTypography.bodyMedium.size)
              .fontColor(MUTED)
              .width('100%')
          }
          .width('100%')
          .padding({ left: 16, right: 16, top: 14, bottom: 14 })
          .backgroundColor(SOFT)
          .borderRadius(20)

          if (this.state.phase === WatchProvisionPhase.Asking) {
            Text(RemoteI18n.t('watchProvision.body'))
              .fontSize(MobileDesignTypography.bodyLarge.size)
              .lineHeight(MobileDesignTypography.bodyLarge.lineHeight)
              .fontColor(MUTED)
              .width('100%')
          } else if (this.state.phase === WatchProvisionPhase.Password) {
            Column({ space: 12 }) {
              Text(RemoteI18n.t('watchProvision.passwordBody'))
                .fontSize(MobileDesignTypography.bodyLarge.size)
                .lineHeight(MobileDesignTypography.bodyLarge.lineHeight)
                .fontColor(MUTED)
                .width('100%')
              TextInput({
                placeholder: RemoteI18n.t('watchProvision.passwordPlaceholder'),
                text: this.password
              })
                .height(54)
                .fontSize(MobileDesignTypography.bodyLarge.size)
                .fontColor(INK)
                .placeholderColor(SUBTLE)
                .backgroundColor(SOFT)
                .borderRadius(18)
                .padding({ left: 18, right: 18 })
                .type(InputType.Password)
                .showPasswordIcon(true)
                .onChange((value: string) => { this.password = value; })
              if (this.state.message.length > 0) {
                Text(this.state.message)
                  .fontSize(MobileDesignTypography.bodySmall.size)
                  .lineHeight(MobileDesignTypography.bodySmall.lineHeight)
                  .fontColor(STATUS_DANGER)
                  .width('100%')
              }
            }
            .width('100%')
          } else if (this.state.phase === WatchProvisionPhase.Working) {
            Row({ space: 10 }) {
              LoadingProgress()
                .width(20)
                .height(20)
                .color(MUTED)
              Text(RemoteI18n.t('watchProvision.working'))
                .fontSize(MobileDesignTypography.bodyLarge.size)
                .lineHeight(MobileDesignTypography.bodyLarge.lineHeight)
                .fontColor(MUTED)
                .layoutWeight(1)
            }
            .width('100%')
          } else {
            Text(this.state.message)
              .fontSize(MobileDesignTypography.bodyLarge.size)
              .lineHeight(MobileDesignTypography.bodyLarge.lineHeight)
              .fontColor(this.state.phase === WatchProvisionPhase.Done ? STATUS_SUCCESS : STATUS_DANGER)
              .width('100%')
          }

          if (this.state.phase === WatchProvisionPhase.Asking) {
            Row({ space: 12 }) {
              Button(RemoteI18n.t('watchProvision.reject'))
                .layoutWeight(1)
                .height(56)
                .fontSize(MobileDesignTypography.labelLarge.size)
                .fontWeight(FontWeight.Bold)
                .fontColor(INK)
                .backgroundColor(SOFT)
                .borderRadius(28)
                .onClick(this.onReject)
              Button(RemoteI18n.t('watchProvision.approve'))
                .layoutWeight(1)
                .height(56)
                .fontSize(MobileDesignTypography.labelLarge.size)
                .fontWeight(FontWeight.Bold)
                .fontColor(CONTENT_ON_ACTION)
                .backgroundColor(PRIMARY_ACTION)
                .borderRadius(28)
                .onClick(() => { this.onApprove(''); })
            }
            .width('100%')
          } else if (this.state.phase === WatchProvisionPhase.Password) {
            Row({ space: 12 }) {
              Button(RemoteI18n.t('watchProvision.reject'))
                .layoutWeight(1)
                .height(56)
                .fontSize(MobileDesignTypography.labelLarge.size)
                .fontWeight(FontWeight.Bold)
                .fontColor(INK)
                .backgroundColor(SOFT)
                .borderRadius(28)
                .onClick(() => {
                  this.password = '';
                  this.onReject();
                })
              Button(RemoteI18n.t('watchProvision.passwordConfirm'))
                .layoutWeight(1)
                .height(56)
                .fontSize(MobileDesignTypography.labelLarge.size)
                .fontWeight(FontWeight.Bold)
                .fontColor(CONTENT_ON_ACTION)
                .backgroundColor(PRIMARY_ACTION)
                .borderRadius(28)
                .enabled(this.password.length > 0)
                .opacity(this.password.length > 0 ? 1 : 0.45)
                .onClick(() => {
                  const password = this.password;
                  this.password = '';
                  this.onApprove(password);
                })
            }
            .width('100%')
          } else if (this.state.phase !== WatchProvisionPhase.Working) {
            Button(RemoteI18n.t('watchProvision.gotIt'))
              .width('100%')
              .height(56)
              .fontSize(MobileDesignTypography.labelLarge.size)
              .fontWeight(FontWeight.Bold)
              .fontColor(CONTENT_ON_ACTION)
              .backgroundColor(PRIMARY_ACTION)
              .borderRadius(28)
              .onClick(this.onDismiss)
          }
        }
        .width('82%')
        .constraintSize({ maxWidth: 520 })
        .padding({ left: 26, right: 26, top: 28, bottom: 26 })
        .backgroundColor(CARD)
        .borderRadius(34)
        .border({ width: 1, color: LINE })
      }
      .width('100%')
      .height('100%')
    }
  }
}
