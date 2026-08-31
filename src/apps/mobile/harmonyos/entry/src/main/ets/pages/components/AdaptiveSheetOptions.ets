import {
  SettingsPlacement,
  SettingsPlacementMode
} from '../policy/SettingsPlacementPolicy';
import { SCRIM, TRANSPARENT } from './Theme';

/**
 * Shared SheetOptions for the four overlay sheets. Policy stays ArkUI-free;
 * this is the only place preferType is chosen.
 */
export class AdaptiveSheetOptions {
  static fromPlacement(placement: SettingsPlacement, dragBar: boolean = false): SheetOptions {
    if (placement.mode === SettingsPlacementMode.FoldOperate) {
      return {
        height: placement.height,
        keyboardAvoidMode: SheetKeyboardAvoidMode.RESIZE_ONLY,
        backgroundColor: TRANSPARENT,
        maskColor: SCRIM,
        showClose: false,
        dragBar
      };
    }
    if (placement.mode === SettingsPlacementMode.Side) {
      return {
        height: placement.height,
        width: placement.width,
        keyboardAvoidMode: SheetKeyboardAvoidMode.RESIZE_ONLY,
        preferType: SheetType.SIDE,
        backgroundColor: TRANSPARENT,
        maskColor: SCRIM,
        showClose: false,
        dragBar: false
      };
    }
    if (placement.mode === SettingsPlacementMode.Center) {
      return {
        height: placement.height,
        width: placement.width,
        keyboardAvoidMode: SheetKeyboardAvoidMode.RESIZE_ONLY,
        preferType: SheetType.CENTER,
        backgroundColor: TRANSPARENT,
        maskColor: SCRIM,
        showClose: false,
        dragBar: false
      };
    }
    if (placement.height > 0) {
      return {
        height: placement.height,
        keyboardAvoidMode: SheetKeyboardAvoidMode.RESIZE_ONLY,
        backgroundColor: TRANSPARENT,
        maskColor: SCRIM,
        showClose: false,
        dragBar
      };
    }
    return {
      height: SheetSize.LARGE,
      keyboardAvoidMode: SheetKeyboardAvoidMode.RESIZE_ONLY,
      backgroundColor: TRANSPARENT,
      maskColor: SCRIM,
      showClose: false,
      dragBar
    };
  }
}
