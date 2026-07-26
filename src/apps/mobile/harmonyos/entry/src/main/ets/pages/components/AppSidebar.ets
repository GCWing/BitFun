import { RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, MUTED, SUBTLE } from './Theme';

@Component
export struct AppSidebar {
  @Prop sessions: RemoteSession[] = [];
  @Prop pinnedSessionId: string = '';
  @Prop selectedSessionId: string = '';
  @Prop connectionState: string = 'idle';
  @Prop activeSection: string = 'chat';
  onClose: () => void = () => {};
  onNewChat: () => void = () => {};
  onEnterCode: () => void = () => {};
  onOpenSettings: () => void = () => {};
  onOpenSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  onArchiveSession: (session: RemoteSession, archived: boolean) => void =
    (_session: RemoteSession, _archived: boolean) => {};
  onExportSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  onDeleteSession: (session: RemoteSession) => void = (_session: RemoteSession) => {};
  @State activeActionSessionId: string = '';
  @State pendingDeleteSessionId: string = '';
  @State showSearch: boolean = false;
  @State sessionSearchQuery: string = '';

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
          .onClick(() => {
            this.showSearch = !this.showSearch;
            if (!this.showSearch) {
              this.sessionSearchQuery = '';
            }
          })
      }
      .width('100%')
      .height(50)
      .alignItems(VerticalAlign.Center)

      if (this.showSearch) {
        TextInput({
          placeholder: RemoteI18n.t('sidebar.searchPlaceholder'),
          text: this.sessionSearchQuery
        })
          .width('100%')
          .height(42)
          .fontSize(14)
          .fontColor(INK)
          .placeholderColor(SUBTLE)
          .padding({ left: 14, right: 14 })
          .backgroundColor('#F4F4F2')
          .borderRadius(8)
          .margin({ top: 12 })
          .onChange((value: string) => {
            this.sessionSearchQuery = value;
          })
      }

      Column({ space: 0 }) {
        this.NavRow(RemoteI18n.t('sidebar.code'), this.activeSection === 'remote', () => {
          this.onEnterCode();
        })
      }
      .width('100%')
      .margin({ top: 18 })

      if (this.visiblePinnedSessions().length > 0) {
        Text('置顶')
          .fontSize(16).fontWeight(FontWeight.Bold).fontColor(INK)
          .width('100%').margin({ top: 18, bottom: 10 })
        ForEach(this.visiblePinnedSessions(), (session: RemoteSession) => {
          this.PinnedRow(session)
        }, (session: RemoteSession) => session.id)
      }

      Text(RemoteI18n.t('sidebar.recent'))
        .fontSize(16)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
        .width('100%')
        .margin({ top: 18, bottom: 10 })

      Stack({ alignContent: Alignment.Bottom }) {
        if (this.visibleSessions().length === 0) {
          Column() {
            this.EmptyRecent()
          }
          .width('100%')
          .height('100%')
        } else {
          List({ space: 2 }) {
            Repeat<RemoteSession>(this.visibleSessions())
              .each((obj: RepeatItem<RemoteSession>) => {
                ListItem() {
                  this.RecentRow(obj)
                }
              })
              .key((item: RemoteSession) => item.id)
              .virtualScroll({ totalCount: this.visibleSessions().length })
          }
          .width('110%')
          .height('100%')
          .margin({ left: -14 })
          .padding({ bottom: 72 })
          .scrollBar(BarState.Off)
          .divider(null)
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
      }
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height('100%')
    .padding({ left: 24, right: 18, top: 4, bottom: 7 })
    .backgroundColor(CARD)
  }

  @Builder
  NavRow(label: string, isActive: boolean, action: () => void) {
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
    .padding({ left: 12, right: 8 })
    .backgroundColor(isActive ? '#F3F3F3' : '#00000000')
    .borderRadius(12)
    .onClick(action)
  }

  @Builder
  PinnedRow(session: RemoteSession) {
    Row({ space: 10 }) {
      Image($r('app.media.remote_actions_check'))
        .width(19).height(19).objectFit(ImageFit.Contain)
      Text(session.title || RemoteI18n.t('sidebar.untitled'))
        .fontSize(17).fontColor(INK).layoutWeight(1)
        .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%').height(46)
    .padding({ left: 14, right: 12 })
    .backgroundColor(this.selectedSessionId === session.id ? '#F3F3F3' : '#00000000')
    .borderRadius(14)
    .onClick(() => this.openSession(session))
  }

  @Builder
  RecentRow(obj: RepeatItem<RemoteSession>) {
    Column({ space: 4 }) {
      Row() {
        Text(obj.item.title || RemoteI18n.t('sidebar.untitled'))
          .fontSize(17)
          .fontWeight(FontWeight.Regular)
          .fontColor(INK)
          .width('100%')
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .width('100%')
      .height(46)
      .padding({ left: 14, right: 12 })
      .backgroundColor(this.selectedSessionId === obj.item.id || this.activeActionSessionId === obj.item.id ?
        '#F3F3F3' : '#00000000')
      .borderRadius(14)
      .onClick(() => {
        this.openSession(obj.item);
      })
      .gesture(
        LongPressGesture({ repeat: false })
          .onAction(() => {
            this.activeActionSessionId = obj.item.id;
            this.pendingDeleteSessionId = '';
          })
      )
      if (this.activeActionSessionId === obj.item.id && this.pendingDeleteSessionId !== obj.item.id) {
        Column({ space: 2 }) {
          if (obj.item.agentType === 'chat') {
            this.SessionAction(obj.item.status === 'archived' ?
              RemoteI18n.t('sidebar.unarchive') : RemoteI18n.t('sidebar.archive'), () => {
              this.activeActionSessionId = '';
              this.onArchiveSession(obj.item, obj.item.status !== 'archived');
            })
            this.SessionAction(RemoteI18n.t('sidebar.exportMarkdown'), () => {
              this.activeActionSessionId = '';
              this.onExportSession(obj.item);
            })
          }
          this.SessionAction(RemoteI18n.t('common.delete'), () => {
            this.pendingDeleteSessionId = obj.item.id;
          }, true)
          this.SessionAction(RemoteI18n.t('common.cancel'), () => {
            this.activeActionSessionId = '';
            this.pendingDeleteSessionId = '';
          })
        }
        .width('100%')
        .padding({ top: 6, bottom: 2 })
      }
      if (this.pendingDeleteSessionId === obj.item.id) {
        Row({ space: 10 }) {
          Text(RemoteI18n.t('sidebar.deleteConfirm'))
            .fontSize(12)
            .fontColor(SUBTLE)
            .layoutWeight(1)
          Text(RemoteI18n.t('common.delete'))
            .fontSize(12)
            .fontColor('#C33B32')
            .height(30)
            .padding({ left: 12, right: 12 })
            .textAlign(TextAlign.Center)
            .backgroundColor('#FFF0EE')
            .borderRadius(15)
            .onClick(() => {
              this.activeActionSessionId = '';
              this.pendingDeleteSessionId = '';
              this.onDeleteSession(obj.item);
            })
        }
        .width('100%')
        .margin({ top: 6 })
        .alignItems(VerticalAlign.Center)
      }
    }
    .width('100%')
  }

  @Builder
  SessionAction(label: string, action: () => void, destructive: boolean = false) {
    Text(label)
      .width('100%')
      .height(34)
      .padding({ left: 12, right: 12 })
      .fontSize(13)
      .fontColor(destructive ? '#C33B32' : INK)
      .textAlign(TextAlign.Start)
      .backgroundColor(destructive ? '#FFF0EE' : '#F4F4F2')
      .borderRadius(6)
      .onClick(action)
  }

  private openSession(item: RemoteSession): void {
    if (item.agentType === 'chat' ||
      this.connectionState === 'connected' || this.connectionState === 'reconnecting') {
      this.onOpenSession(item);
      return;
    }
    this.onEnterCode();
  }

  @Builder
  EmptyRecent() {
    Column() {
      Text(this.sessionSearchQuery.trim().length > 0 ?
        RemoteI18n.t('sidebar.noSearchResult') : RemoteI18n.t('sidebar.emptyRecentTitle'))
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
    if (this.connectionState === 'connected' || this.connectionState === 'reconnecting') {
      Image($r('app.media.remote_ref_sidebar_connected'))
        .width(35)
        .height(34)
        .objectFit(ImageFit.Contain)
    } else {
      Image($r('app.media.remote_logo'))
        .width(34)
        .height(34)
        .objectFit(ImageFit.Contain)
    }
  }

  @Builder
  SearchGlyph() {
    Image($r('app.media.sidebar_ref_search_reference'))
      .width(23)
      .height(23)
      .objectFit(ImageFit.Contain)
      .translate({ x: -2.5, y: 1.5 })
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
    Image($r('app.media.remote_ref_new_chat'))
      .width(25)
      .height(25)
      .objectFit(ImageFit.Contain)
  }

  @Builder
  SettingsGlyph() {
    Image($r('app.media.sidebar_ref_settings_reference'))
      .width(24.5)
      .height(25)
      .objectFit(ImageFit.Contain)
      .translate({ x: -5 })
  }

  private visibleSessions(): RemoteSession[] {
    const query = this.sessionSearchQuery.trim().toLowerCase();
    return this.sessions.filter((session: RemoteSession) => {
      if (session.agentType !== 'chat') {
        return false;
      }
      if (session.id === this.pinnedSessionId) {
        return false;
      }
      if (query.length > 0) {
        return session.title.toLowerCase().indexOf(query) >= 0;
      }
      return session.status !== 'archived';
    });
  }

  private visiblePinnedSessions(): RemoteSession[] {
    const query = this.sessionSearchQuery.trim().toLowerCase();
    return this.sessions.filter((session: RemoteSession) => {
      return session.agentType === 'chat' && session.id === this.pinnedSessionId &&
        session.status !== 'archived' &&
        (query.length === 0 || session.title.toLowerCase().indexOf(query) >= 0);
    });
  }
}
