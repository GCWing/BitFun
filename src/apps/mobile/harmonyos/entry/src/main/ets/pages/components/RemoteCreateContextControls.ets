import { MobileDesignTypography } from '../../generated/MobileDesignTokens';
import { RecentWorkspaceEntry } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { RemoteCreateSessionState, RemoteHarnessProfile } from '../state/RemoteCreateSessionState';
import { CARD, INK, LINE, MUTED, SOFT } from './Theme';
import { ComposerPresentation } from './ComposerBar';

/**
 * Remote-session creation context rendered immediately above the composer.
 *
 * Keep this as a real component instead of injecting a parent-owned builder
 * into ConversationView. ArkUI can otherwise compile the slot successfully
 * while dropping its nodes on device because the V2 render owner differs.
 */
@ComponentV2
export struct RemoteCreateContextControls {
  @Param state: RemoteCreateSessionState = new RemoteCreateSessionState();
  @Param presentation: ComposerPresentation = ComposerPresentation.Create;
  @Event onToggleDeviceMenu: () => void = () => {};
  @Event onToggleWorkspaceMenu: () => void = () => {};
  @Event onSelectDevice: (device: CloudAccountDevice) => void = (_device: CloudAccountDevice) => {};
  @Event onSelectWorkspace: (workspace?: RecentWorkspaceEntry) => void =
    (_workspace?: RecentWorkspaceEntry) => {};
  @Event onSelectHarness: (profile: RemoteHarnessProfile) => void = (_profile: RemoteHarnessProfile) => {};
  @Local showSelectorSheet: boolean = false;

  build() {
    Column({ space: 2 }) {
      if (this.presentation === ComposerPresentation.Floating) {
        this.ContextRow('device', this.state.isLoadingDevices, () => this.onToggleDeviceMenu())
      }
      this.ContextRow('workspace', this.state.isLoadingWorkspaces, () => this.onToggleWorkspaceMenu())
      if (this.state.supportsHarnessProfiles && this.state.selectedWorkspacePath.length > 0) {
        this.HarnessSelector()
      }
    }
    .width('100%')
    .padding({
      left: this.presentation === ComposerPresentation.Floating ? 10 : 28,
      right: this.presentation === ComposerPresentation.Floating ? 10 : 28,
      top: this.presentation === ComposerPresentation.Floating ? 8 : 0,
      bottom: 4
    })
    .bindSheet($$this.showSelectorSheet, this.SelectorSheet(), this.selectorSheetOptions())
  }

  @Builder
  private HarnessSelector() {
    Row({ space: 3 }) {
      this.HarnessOption('minimal', 1)
      this.HarnessOption('balanced', 2)
      this.HarnessOption('ultimate', 3)
    }
    .width('100%')
    .height(42)
    .padding(3)
    .borderRadius(12)
    .backgroundColor(SOFT)
  }

  @Builder
  private HarnessOption(profile: RemoteHarnessProfile, density: number) {
    Row({ space: 7 }) {
      this.HarnessDensityMark(density, profile === this.state.selectedHarnessProfile)
      Text(RemoteI18n.t(`remote.create.harness.${profile}`))
        .fontSize(MobileDesignTypography.bodyMedium.size)
        .fontWeight(profile === this.state.selectedHarnessProfile ? FontWeight.Medium : FontWeight.Regular)
        .fontColor(profile === this.state.selectedHarnessProfile ? INK : MUTED)
    }
    .layoutWeight(1)
    .height(36)
    .justifyContent(FlexAlign.Center)
    .borderRadius(9)
    .backgroundColor(profile === this.state.selectedHarnessProfile ? CARD : SOFT)
    .accessibilityText(RemoteI18n.f(
      'remote.create.harnessAccessibility',
      RemoteI18n.t(`remote.create.harness.${profile}`)
    ))
    .onClick(() => {
      this.onSelectHarness(profile);
      this.keepComposerFocused();
    })
  }

  @Builder
  private HarnessDensityMark(density: number, selected: boolean) {
    Row({ space: 2 }) {
      Stack().width(3).height(8).borderRadius(2).backgroundColor(selected ? INK : MUTED)
      if (density >= 2) {
        Stack().width(3).height(12).borderRadius(2).backgroundColor(selected ? INK : MUTED)
      }
      if (density >= 3) {
        Stack().width(3).height(16).borderRadius(2).backgroundColor(selected ? INK : MUTED)
      }
    }
    .width(17)
    .height(18)
    .alignItems(VerticalAlign.Bottom)
    .justifyContent(FlexAlign.Center)
  }

  @Builder
  private ContextRow(kind: string, loading: boolean, onClick: () => void) {
    Row({ space: 13 }) {
      this.ContextGlyph(kind, loading)
      Text(loading ? RemoteI18n.t('common.loading') :
        (kind === 'device' ?
          (this.state.selectedDeviceName || RemoteI18n.t('remote.create.noDevice')) :
          (this.state.selectedWorkspaceName || RemoteI18n.t('remote.create.chat'))))
        .constraintSize({ maxWidth: '74%' })
        .fontSize(MobileDesignTypography.bodyLarge.size)
        .fontWeight(FontWeight.Medium)
        .fontColor(loading ? MUTED : INK)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      SymbolGlyph(this.state.openMenu === (kind === 'device' ? 'devices' : 'workspaces') ?
        $r('sys.symbol.chevron_up') : $r('sys.symbol.chevron_down'))
        .fontSize(13)
        .fontColor([MUTED])
        .width(22)
        .height(30)
        .opacity(loading ? 0.42 : 1)
      Blank().layoutWeight(1)
    }
    .width('100%')
    .height(48)
    .padding({ left: 8, right: 10 })
    .borderRadius(12)
    .bindPopup(this.state.openMenu === (kind === 'device' ? 'devices' : 'workspaces'), {
      builder: () => {
        if (kind === 'device') {
          this.DeviceMenu(false)
        } else {
          this.WorkspaceMenu(false)
        }
      },
      placement: Placement.Top,
      popupColor: '#00000000',
      enableArrow: false,
      autoCancel: true,
      mask: false,
      targetSpace: 8,
      onStateChange: (event) => {
        if (!event.isVisible) {
          this.state.closeMenu();
        }
      }
    })
    .onClick(() => {
      onClick();
      this.keepComposerFocused();
    })
  }

  @Builder
  private ContextGlyph(kind: string, loading: boolean) {
    if (kind === 'device') {
      SymbolGlyph($r('sys.symbol.desktop'))
        .fontSize(22).fontColor([MUTED]).width(26).height(26).opacity(loading ? 0.42 : 1)
    } else if (this.state.selectedWorkspacePath.length > 0) {
      SymbolGlyph($r('sys.symbol.folder'))
        .fontSize(22).fontColor([MUTED]).width(26).height(26).opacity(loading ? 0.42 : 1)
    } else {
      SymbolGlyph($r('sys.symbol.message'))
        .fontSize(22).fontColor([MUTED]).width(26).height(26).opacity(loading ? 0.42 : 1)
    }
  }

  @Builder
  private DeviceMenu(asSheet: boolean) {
    Column() {
      if (this.state.devices.length === 0 && this.state.isLoadingDevices) {
        this.MenuMessage(RemoteI18n.t('common.loading'))
      } else if (this.state.devices.length === 0) {
        this.MenuMessage(RemoteI18n.t('remote.create.noOnlineDevice'))
      } else {
        ForEach(this.state.devices, (device: CloudAccountDevice) => {
          this.DeviceMenuRow(device)
        }, (device: CloudAccountDevice) => device.deviceId)
      }
    }
    .width(asSheet ? '100%' : (this.presentation === ComposerPresentation.Floating ? 360 : 340))
    .padding({ top: 8, bottom: 8 })
    .backgroundColor(CARD)
    .borderRadius(asSheet ? 0 : 16)
    .border({ width: 1, color: LINE })
    .shadow({ radius: asSheet ? 0 : 20, color: asSheet ? '#00000000' : '#1A000000', offsetY: 8 })
  }

  @Builder
  private DeviceMenuRow(device: CloudAccountDevice) {
    Row({ space: 12 }) {
      this.SelectionMark(device.deviceId === this.state.selectedDeviceId)
      SymbolGlyph($r('sys.symbol.desktop')).fontSize(22).fontColor([MUTED]).width(27).height(27)
      Text(device.deviceName)
        .layoutWeight(1)
        .fontSize(MobileDesignTypography.bodyLarge.size)
        .fontColor(INK)
        .maxLines(2)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(58)
    .padding({ left: 16, right: 18, top: 8, bottom: 8 })
    .onClick(() => {
      this.showSelectorSheet = false;
      this.onSelectDevice(device);
      this.keepComposerFocused();
    })
  }

  @Builder
  private WorkspaceMenu(asSheet: boolean) {
    Column() {
      Row({ space: 12 }) {
        this.SelectionMark(this.state.selectedWorkspacePath.length === 0)
        SymbolGlyph($r('sys.symbol.message')).fontSize(22).fontColor([MUTED]).width(27).height(27)
        Text(RemoteI18n.t('remote.create.chat')).layoutWeight(1).fontSize(MobileDesignTypography.bodyLarge.size).fontColor(INK)
      }
      .width('100%')
      .height(58)
      .padding({ left: 16, right: 18 })
      .onClick(() => {
        this.showSelectorSheet = false;
        this.onSelectWorkspace(undefined);
        this.keepComposerFocused();
      })

      Divider().color(LINE).margin({ left: 58, right: 18 })

      if (this.state.workspaces.length === 0 && this.state.isLoadingWorkspaces) {
        this.MenuMessage(RemoteI18n.t('common.loading'))
      } else if (this.state.workspaces.length === 0) {
        this.MenuMessage(RemoteI18n.t('home.noRecentWorkspaces'))
      } else {
        Scroll() {
          Column() {
            ForEach(this.state.workspaces, (workspace: RecentWorkspaceEntry) => {
              this.WorkspaceMenuRow(workspace)
            }, (workspace: RecentWorkspaceEntry) => workspace.path)
          }
        }
        .width('100%')
        .constraintSize({ maxHeight: this.presentation === ComposerPresentation.Floating ? 190 : 92 })
        .scrollBar(BarState.Off)
      }
    }
    .width(asSheet ? '100%' : (this.presentation === ComposerPresentation.Floating ? 360 : 340))
    .padding({ top: 8, bottom: 8 })
    .backgroundColor(CARD)
    .borderRadius(asSheet ? 0 : 16)
    .border({ width: 1, color: LINE })
    .shadow({ radius: asSheet ? 0 : 20, color: asSheet ? '#00000000' : '#1A000000', offsetY: 8 })
  }

  @Builder
  private WorkspaceMenuRow(workspace: RecentWorkspaceEntry) {
    Row({ space: 12 }) {
      this.SelectionMark(workspace.path === this.state.selectedWorkspacePath)
      SymbolGlyph($r('sys.symbol.folder')).fontSize(22).fontColor([MUTED]).width(27).height(27)
      Column({ space: 2 }) {
        Text(workspace.name)
          .width('100%').fontSize(MobileDesignTypography.bodyLarge.size).fontColor(INK).maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(workspace.path)
          .width('100%').fontSize(MobileDesignTypography.labelSmall.size).fontColor(MUTED).maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .height(64)
    .padding({ left: 16, right: 18 })
    .onClick(() => {
      this.showSelectorSheet = false;
      this.onSelectWorkspace(workspace);
      this.keepComposerFocused();
    })
  }

  @Builder
  private SelectionMark(selected: boolean) {
    Stack({ alignContent: Alignment.Center }) {
      if (selected) {
        SymbolGlyph($r('sys.symbol.checkmark_circle')).fontSize(19).fontColor([INK])
      }
    }
    .width(20)
    .height(24)
  }

  @Builder
  private MenuMessage(message: string) {
    Text(message)
      .width('100%')
      .padding({ left: 20, right: 20, top: 18, bottom: 18 })
      .fontSize(MobileDesignTypography.bodySmall.size)
      .fontColor(MUTED)
  }

  @Builder
  private SelectorSheet() {
    Column() {
      Row() {
        Text(this.state.openMenu === 'devices' ? RemoteI18n.t('remote.device') : RemoteI18n.t('remote.workspace'))
          .layoutWeight(1).fontSize(MobileDesignTypography.bodyLarge.size).fontWeight(FontWeight.Medium).fontColor(INK)
        Stack({ alignContent: Alignment.Center }) {
          SymbolGlyph($r('sys.symbol.xmark')).fontSize(16).fontColor([MUTED])
        }
        .width(44)
        .height(44)
        .accessibilityText(RemoteI18n.t('common.close'))
        .onClick(() => this.closeSelectorSheet())
      }
      .width('100%')
      .height(52)
      .padding({ left: 18, right: 8 })
      if (this.state.openMenu === 'devices') {
        this.DeviceMenu(true)
      } else {
        this.WorkspaceMenu(true)
      }
    }
    .width('100%')
    .padding({ left: 12, right: 12, bottom: 16 })
    .backgroundColor(CARD)
    .borderRadius(16)
  }

  private keepComposerFocused(): void {
    setTimeout(() => focusControl.requestFocus('remote-create-composer'), 30);
  }

  private closeSelectorSheet(): void {
    this.showSelectorSheet = false;
    this.state.closeMenu();
    this.keepComposerFocused();
  }

  private selectorSheetOptions(): SheetOptions {
    return {
      height: this.state.openMenu === 'devices' ? 420 : 440,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: true,
      onWillDismiss: () => {
        this.state.closeMenu();
      }
    };
  }
}
