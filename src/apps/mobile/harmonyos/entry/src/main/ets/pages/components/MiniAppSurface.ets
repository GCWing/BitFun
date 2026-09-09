import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { MiniAppActions, RemoteMiniAppSummary, emptyMiniAppActions } from '../../model/RemoteMiniAppModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { MiniAppState } from '../state/MiniAppState';
import { MiniAppWebView } from './MiniAppWebView';
import { PAGE_BG, INK, MUTED, LINE } from './Theme';

@ComponentV2
export struct MiniAppSurface {
  @Param state: MiniAppState = new MiniAppState();
  @Param actions: MiniAppActions = emptyMiniAppActions();

  build() {
    Column() {
      Row({ space: 16 }) {
        Button(RemoteI18n.t(this.state.appId ? 'common.back' : 'common.close'))
          .onClick(() => this.state.appId ? this.actions.back() : this.actions.close())
        Text(this.state.name || RemoteI18n.t('miniapp.title'))
          .fontSize(MobileDesignTypography.titleMedium.size).fontColor(INK).maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis }).layoutWeight(1)
        if (!this.state.appId) {
          Button(RemoteI18n.t('miniapp.refresh')).onClick(() => this.actions.refresh()).enabled(!this.state.loading)
        }
      }.padding(16).width('100%')
      Text(RemoteI18n.t('miniapp.previewHint')).fontSize(MobileDesignTypography.bodySmall.size).fontColor(MUTED).padding({ left: 16, right: 16, bottom: 10 })
      if (this.state.loading) {
        LoadingProgress().width(32).height(32).margin(24)
      } else if (this.state.error) {
        Text(this.state.error).fontColor(INK).padding(24).copyOption(CopyOptions.InApp)
      } else if (this.state.document) {
        Column() {
          MiniAppWebView({ document: this.state.document, revision: this.state.revision, request: this.actions.request })
        }.width('100%').layoutWeight(1)
      } else if (this.state.apps.length === 0) {
        Text(RemoteI18n.t('miniapp.empty')).fontColor(MUTED).padding(24)
      } else {
        List({ space: 8 }) {
          ForEach(this.state.apps, (app: RemoteMiniAppSummary) => {
            ListItem() {
              Column({ space: 6 }) {
                Text(app.name).fontSize(MobileDesignTypography.titleMedium.size).fontColor(INK)
                Text(app.description).fontSize(MobileDesignTypography.bodySmall.size).fontColor(MUTED).maxLines(2)
              }.alignItems(HorizontalAlign.Start).padding(16).width('100%')
                .border({ width: 1, color: LINE }).borderRadius(12)
                .onClick(() => this.actions.select(app.id))
            }
          }, (app: RemoteMiniAppSummary) => app.id)
        }.padding({ left: 16, right: 16 }).layoutWeight(1).width('100%')
      }
    }.width('100%').height('100%').backgroundColor(PAGE_BG)
  }
}
