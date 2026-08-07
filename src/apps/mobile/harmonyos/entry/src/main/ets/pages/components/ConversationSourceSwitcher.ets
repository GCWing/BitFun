import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConversationSource } from '../navigation/AppRouteContract';
import { CARD, INK, LINE, MUTED, SOFT } from './Theme';

@Component
export struct ConversationSourceSwitcher {
  @Prop activeSource: ConversationSource = ConversationSource.General;
  onSelectSource: (source: ConversationSource) => void = (_source: ConversationSource) => {};

  build() {
    Row({ space: 2 }) {
      this.SourceOption(ConversationSource.General, RemoteI18n.t('sidebar.local'))
      this.SourceOption(ConversationSource.Remote, RemoteI18n.t('sidebar.code'))
    }
    .width('100%')
    .height(40)
    .padding(3)
    .backgroundColor(SOFT)
    .borderRadius(8)
    .border({ width: 1, color: LINE })
  }

  @Builder
  private SourceOption(source: ConversationSource, label: string) {
    Text(label)
      .layoutWeight(1)
      .height(32)
      .fontSize(13)
      .fontWeight(FontWeight.Medium)
      .fontColor(this.activeSource === source ? INK : MUTED)
      .textAlign(TextAlign.Center)
      .backgroundColor(this.activeSource === source ? CARD : '#00000000')
      .borderRadius(6)
      .onClick(() => {
        if (this.activeSource !== source) {
          this.onSelectSource(source);
        }
      })
  }
}
