import { RemoteI18n } from '../../i18n/RemoteI18n';
import { MUTED } from './Theme';

@Component
export struct ThinkingBlock {
  @Prop text: string = '';
  @Prop status: string = '';
  @Prop keepExpandedWhenDone: boolean = false;
  @Prop streaming: boolean = false;
  @Prop streamKey: string = '';
  onCopyText: (text: string) => void = (_text: string) => {};

  build() {
    if (this.isRunning()) {
      Row({ space: 6 }) {
        Text('•••')
          .fontSize(14)
          .fontColor('#2F80ED')
          .letterSpacing(0)
        Text(RemoteI18n.t('chat.thinkingInProgress'))
          .fontSize(14)
          .fontColor(MUTED)
        Text('•••')
          .fontSize(14)
          .fontColor('#2F80ED')
          .letterSpacing(0)
      }
      .width('100%')
      .height(28)
      .padding({ left: 0, right: 0, top: 2, bottom: 2 })
    }
  }

  private isRunning(): boolean {
    const normalized = (this.status || '').toLowerCase();
    return normalized === 'active' || normalized === 'running';
  }
}
