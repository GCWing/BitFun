import { RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { TimeFormat } from '../../services/TimeFormat';
import { CARD, INK, LINE, MUTED, SUBTLE } from './Theme';

@Component
export struct AppSidebar {
  @Prop sessions: RemoteSession[] = [];
  @Prop connectionState: string = 'idle';
  onClose: () => void = () => {};
  onNewChat: () => void = () => {};
  onEnterCode: () => void = () => {};
  onOpenSettings: () => void = () => {};
  onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};

  build() {
    Column() {
      Row() {
        Text('BitFun')
          .fontSize(23)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
        Blank()
        Stack({ alignContent: Alignment.Center }) {
          this.SearchGlyph()
        }
          .width(46)
          .height(46)
          .backgroundColor(CARD)
          .borderRadius(23)
          .shadow({ radius: 18, color: '#10000000', offsetY: 8 })
      }
      .width('100%')
      .height(50)
      .alignItems(VerticalAlign.Center)

      Column({ space: 0 }) {
        this.NavRow(RemoteI18n.t('sidebar.code'), () => {
          this.onEnterCode();
        })
      }
      .width('100%')
      .margin({ top: 18 })

      Text(RemoteI18n.t('sidebar.recent'))
        .fontSize(16)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .width('100%')
        .margin({ top: 18, bottom: 10 })

      if (this.sessions.length === 0) {
        this.EmptyRecent()
        Blank()
          .layoutWeight(1)
      } else {
        Scroll() {
          Column({ space: 2 }) {
            ForEach(this.visibleSessions(), (item: RemoteSession) => {
              this.RecentRow(item)
            }, (item: RemoteSession) => item.id)
          }
          .width('100%')
        }
        .layoutWeight(1)
        .scrollBar(BarState.Off)
      }

      Row() {
        Button() {
          Row({ space: 10 }) {
            this.EditGlyph()
            Text(RemoteI18n.t('sidebar.newChat'))
              .fontSize(15)
              .fontWeight(FontWeight.Medium)
              .fontColor(CARD)
          }
          .justifyContent(FlexAlign.Center)
          .width('100%')
        }
        .width(116)
        .height(44)
        .backgroundColor('#3B82F6')
        .borderRadius(22)
        .onClick(() => {
          this.onNewChat();
        })

        Blank()

        Stack({ alignContent: Alignment.Center }) {
          this.SettingsGlyph()
        }
          .width(46)
          .height(46)
          .backgroundColor(CARD)
          .borderRadius(23)
          .shadow({ radius: 16, color: '#0E000000', offsetY: 8 })
          .onClick(() => {
            this.onOpenSettings();
          })
      }
      .width('100%')
      .alignItems(VerticalAlign.Center)
      .margin({ top: 16 })
    }
    .width('76%')
    .height('100%')
    .padding({ left: 24, right: 18, top: 4, bottom: 24 })
    .backgroundColor(CARD)
  }

  @Builder
  NavRow(label: string, action: () => void) {
    Row({ space: 14 }) {
      this.RemoteGlyph()
      Text(label)
        .fontSize(18)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .layoutWeight(1)
    }
    .width('100%')
    .height(46)
    .padding({ left: 2, right: 8 })
    .backgroundColor('#00000000')
    .onClick(action)
  }

  @Builder
  RecentRow(item: RemoteSession) {
    Column({ space: 4 }) {
      Text(item.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(19)
        .fontColor(INK)
        .width('100%')
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      Text(`${this.displayAgent(item.agentType)} · ${TimeFormat.relative(item.updatedAt || item.createdAt)}`)
        .fontSize(12)
        .fontColor(SUBTLE)
        .width('100%')
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .padding({ top: 12, bottom: 12 })
    .border({ width: { bottom: 1 }, color: LINE })
    .onClick(() => {
      if (this.connectionState === 'connected' || this.connectionState === 'reconnecting') {
        this.onOpenSession(item);
      } else {
        this.onEnterCode();
      }
    })
  }

  @Builder
  EmptyRecent() {
    Column() {
      Text(RemoteI18n.t('sidebar.emptyRecentTitle'))
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
        .width('100%')
    }
    .width('100%')
    .padding({ top: 8, bottom: 8 })
  }

  @Builder
  RemoteGlyph() {
    Image($r('app.media.remote_logo'))
      .width(34)
      .height(34)
      .objectFit(ImageFit.Contain)
  }

  @Builder
  SearchGlyph() {
    Image($r('app.media.gpt_search_logo'))
      .width(46)
      .height(46)
      .objectFit(ImageFit.Contain)
  }

  @Builder
  LibraryGlyph() {
    Row({ space: 3 }) {
      this.BookSpine()
      this.BookSpine()
      this.BookSpine()
    }
    .width(30)
    .height(30)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  BookSpine() {
    Text('')
      .width(6)
      .height(24)
      .borderRadius(3)
      .border({ width: 1.5, color: INK })
  }

  @Builder
  NotebookGlyph() {
    Stack() {
      Text('')
        .width(22)
        .height(24)
        .borderRadius(5)
        .border({ width: 1.5, color: INK })
        .position({ x: 8, y: 5 })
      Text('')
        .width(4)
        .height(4)
        .borderRadius(2)
        .backgroundColor(INK)
        .position({ x: 5, y: 11 })
      Text('')
        .width(4)
        .height(4)
        .borderRadius(2)
        .backgroundColor(INK)
        .position({ x: 5, y: 20 })
    }
    .width(34)
    .height(34)
  }

  @Builder
  ClockGlyph() {
    Stack() {
      Text('')
        .width(26)
        .height(26)
        .borderRadius(13)
        .border({ width: 1.5, color: INK })
        .position({ x: 4, y: 4 })
      Text('')
        .width(1.5)
        .height(9)
        .backgroundColor(INK)
        .borderRadius(2)
        .position({ x: 18, y: 10 })
      Text('')
        .width(9)
        .height(1.5)
        .backgroundColor(INK)
        .borderRadius(2)
        .position({ x: 18, y: 20 })
    }
    .width(34)
    .height(34)
  }

  @Builder
  AppsGlyph() {
    Column({ space: 8 }) {
      Row({ space: 8 }) {
        this.AppDot()
        this.AppDot()
      }
      Row({ space: 8 }) {
        this.AppDot()
        this.AppDot()
      }
    }
    .width(24)
    .height(24)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  AppDot() {
    Text('')
      .width(8)
      .height(8)
      .borderRadius(4)
      .backgroundColor(INK)
  }

  @Builder
  CodeFlowerGlyph() {
    Stack() {
      Text('')
        .width(16)
        .height(16)
        .borderRadius(8)
        .border({ width: 1.5, color: INK })
        .backgroundColor(CARD)
        .position({ x: 9, y: 1 })
      Text('')
        .width(16)
        .height(16)
        .borderRadius(8)
        .border({ width: 1.5, color: INK })
        .backgroundColor(CARD)
        .position({ x: 17, y: 9 })
      Text('')
        .width(16)
        .height(16)
        .borderRadius(8)
        .border({ width: 1.5, color: INK })
        .backgroundColor(CARD)
        .position({ x: 9, y: 17 })
      Text('')
        .width(16)
        .height(16)
        .borderRadius(8)
        .border({ width: 1.5, color: INK })
        .backgroundColor(CARD)
        .position({ x: 1, y: 9 })
      Text('')
        .width(14)
        .height(14)
        .borderRadius(7)
        .backgroundColor(CARD)
        .position({ x: 10, y: 10 })
    }
    .width(34)
    .height(34)
  }

  @Builder
  MoreGlyph() {
    Row({ space: 5 }) {
      this.DotGlyph()
      this.DotGlyph()
      this.DotGlyph()
    }
    .width(30)
    .height(22)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  DotGlyph() {
    Text('')
      .width(5)
      .height(5)
      .borderRadius(3)
      .backgroundColor(INK)
  }

  @Builder
  EditGlyph() {
    Image($r('app.media.gpt_chat_glyph'))
      .width(34)
      .height(34)
      .objectFit(ImageFit.Contain)
  }

  @Builder
  SettingsGlyph() {
    Image($r('app.media.gpt_settings_logo'))
      .width(46)
      .height(46)
      .objectFit(ImageFit.Contain)
  }

  private visibleSessions(): RemoteSession[] {
    return this.sessions.slice(0, 8);
  }

  private displayAgent(agentType: string): string {
    const normalized = (agentType || '').toLowerCase();
    if (normalized === 'cowork') {
      return 'Cowork';
    }
    return 'Code';
  }
}
