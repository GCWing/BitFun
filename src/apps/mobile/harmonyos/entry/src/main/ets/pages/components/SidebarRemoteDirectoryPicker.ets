import { RemoteDirectoryEntry, RemoteDirectoryListing } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import {
  CARD,
  INK,
  LINE,
  MUTED,
  PAGE_BG,
  PRIMARY_ACTION,
  PRIMARY_ACTION_TEXT,
  RED,
  SOFT
} from './Theme';

@ComponentV2
export struct SidebarRemoteDirectoryPicker {
  @Param deviceName: string = '';
  @Event onClose: () => void = () => {};
  @Event onLoadDirectories: (path: string) => Promise<RemoteDirectoryListing> =
    async (_path: string): Promise<RemoteDirectoryListing> => ({
      path: '', parentPath: '', directories: [], truncated: false
    });
  @Event onSelectDirectory: (path: string) => void = (_path: string) => {};
  @Local currentPath: string = '';
  @Local parentPath: string = '';
  @Local directories: RemoteDirectoryEntry[] = [];
  @Local loading: boolean = true;
  @Local errorText: string = '';
  @Local truncated: boolean = false;
  private requestVersion: number = 0;

  aboutToAppear(): void {
    void this.loadDirectory('');
  }

  build() {
    Column() {
      this.Header()
      Divider().color(LINE)
      this.PathBar()
      if (this.loading) {
        this.LoadingState()
      } else if (this.errorText.length > 0) {
        this.ErrorState()
      } else {
        this.DirectoryList()
      }
      if (!this.loading && this.errorText.length === 0 && this.currentPath.length > 0) {
        this.ChooseButton()
      }
    }
    .width('100%')
    .height('100%')
    .padding({ left: 18, right: 18, top: 10, bottom: 12 })
    .backgroundColor(PAGE_BG)
  }

  @Builder
  private Header() {
    Row({ space: 12 }) {
      Column({ space: 2 }) {
        Text(RemoteI18n.t('sidebar.selectRemoteFolder'))
          .width('100%')
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
        Text(this.deviceName)
          .width('100%')
          .fontSize(13)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.xmark'))
          .fontSize(17)
          .fontColor([MUTED])
      }
      .width(40)
      .height(40)
      .borderRadius(20)
      .backgroundColor(SOFT)
      .accessibilityText(RemoteI18n.t('common.close'))
      .onClick(() => this.onClose())
    }
    .width('100%')
    .height(66)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private PathBar() {
    Column({ space: 4 }) {
      Text(RemoteI18n.t('sidebar.currentFolder'))
        .width('100%')
        .fontSize(11)
        .fontColor(MUTED)
      Text(this.currentPath.length > 0 ? this.currentPath :
        (this.loading ? RemoteI18n.t('common.loading') : RemoteI18n.t('sidebar.remoteHomeFolder')))
        .width('100%')
        .fontSize(13)
        .fontColor(INK)
        .maxLines(2)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(58)
    .padding({ left: 12, right: 12, top: 9, bottom: 8 })
    .margin({ top: 10, bottom: 4 })
    .alignItems(HorizontalAlign.Start)
    .justifyContent(FlexAlign.Center)
    .backgroundColor(SOFT)
    .borderRadius(12)
  }

  @Builder
  private DirectoryList() {
    if (this.directories.length === 0 && this.parentPath.length === 0) {
      this.EmptyState()
    } else {
      Scroll() {
        Column({ space: 4 }) {
          if (this.parentPath.length > 0) {
            this.ParentRow()
          }
          ForEach(this.directories, (directory: RemoteDirectoryEntry) => {
            this.DirectoryRow(directory)
          }, (directory: RemoteDirectoryEntry): string => directory.path)
          if (this.directories.length === 0) {
            Text(RemoteI18n.t('sidebar.emptyFolders'))
              .width('100%')
              .fontSize(13)
              .fontColor(MUTED)
              .textAlign(TextAlign.Center)
              .padding({ top: 28, bottom: 18 })
          }
          if (this.truncated) {
            Text(RemoteI18n.t('sidebar.directoryTruncated'))
              .width('100%')
              .fontSize(12)
              .fontColor(MUTED)
              .textAlign(TextAlign.Center)
              .padding({ top: 10, bottom: 12 })
          }
        }
        .width('100%')
        .padding({ top: 8, bottom: 14 })
      }
      .width('100%')
      .layoutWeight(1)
      .scrollBar(BarState.Off)
    }
  }

  @Builder
  private ParentRow() {
    Row({ space: 12 }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.arrow_up'))
          .fontSize(18)
          .fontColor([INK])
      }
      .width(34)
      .height(34)
      .borderRadius(10)
      .backgroundColor(CARD)
      Text(RemoteI18n.t('sidebar.parentFolder'))
        .fontSize(15)
        .fontColor(INK)
        .layoutWeight(1)
      SymbolGlyph($r('sys.symbol.chevron_right'))
        .fontSize(14)
        .fontColor([MUTED])
    }
    .width('100%')
    .height(58)
    .padding({ left: 10, right: 10 })
    .borderRadius(14)
    .onClick(() => void this.loadDirectory(this.parentPath))
  }

  @Builder
  private DirectoryRow(directory: RemoteDirectoryEntry) {
    Row({ space: 12 }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.folder'))
          .fontSize(20)
          .fontColor([INK])
      }
      .width(34)
      .height(34)
      .borderRadius(10)
      .backgroundColor(CARD)
      Text(directory.name)
        .fontSize(15)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      SymbolGlyph($r('sys.symbol.chevron_right'))
        .fontSize(14)
        .fontColor([MUTED])
    }
    .width('100%')
    .height(58)
    .padding({ left: 10, right: 10 })
    .borderRadius(14)
    .onClick(() => void this.loadDirectory(directory.path))
  }

  @Builder
  private ChooseButton() {
    Text(RemoteI18n.t('sidebar.chooseThisFolder'))
      .width('100%')
      .height(48)
      .fontSize(16)
      .fontWeight(FontWeight.Medium)
      .fontColor(PRIMARY_ACTION_TEXT)
      .textAlign(TextAlign.Center)
      .backgroundColor(PRIMARY_ACTION)
      .borderRadius(14)
      .onClick(() => this.onSelectDirectory(this.currentPath))
  }

  @Builder
  private LoadingState() {
    Column({ space: 12 }) {
      LoadingProgress().width(22).height(22).color(MUTED)
      Text(RemoteI18n.t('sidebar.loadingFolders')).fontSize(14).fontColor(MUTED)
    }
    .width('100%')
    .layoutWeight(1)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  private EmptyState() {
    Column({ space: 8 }) {
      SymbolGlyph($r('sys.symbol.folder')).fontSize(30).fontColor([MUTED])
      Text(RemoteI18n.t('sidebar.emptyFolders'))
        .fontSize(14)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
    }
    .width('100%')
    .layoutWeight(1)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  private ErrorState() {
    Column({ space: 10 }) {
      Text(this.errorText)
        .fontSize(14)
        .fontColor(RED)
        .textAlign(TextAlign.Center)
      Text(RemoteI18n.t('common.retry'))
        .height(42)
        .padding({ left: 22, right: 22 })
        .fontSize(14)
        .fontColor(INK)
        .textAlign(TextAlign.Center)
        .backgroundColor(SOFT)
        .borderRadius(21)
        .onClick(() => void this.loadDirectory(this.currentPath))
    }
    .width('100%')
    .layoutWeight(1)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  private async loadDirectory(path: string): Promise<void> {
    const version = ++this.requestVersion;
    this.loading = true;
    this.errorText = '';
    try {
      const listing = await this.onLoadDirectories(path);
      if (version !== this.requestVersion) {
        return;
      }
      this.currentPath = listing.path;
      this.parentPath = listing.parentPath;
      this.directories = listing.directories;
      this.truncated = listing.truncated;
    } catch (err) {
      if (version !== this.requestVersion) {
        return;
      }
      const detail = String(err).replace(/^Error:\s*/, '').trim();
      this.errorText = detail || RemoteI18n.t('sidebar.directoryLoadFailed');
    } finally {
      if (version === this.requestVersion) {
        this.loading = false;
      }
    }
  }
}
