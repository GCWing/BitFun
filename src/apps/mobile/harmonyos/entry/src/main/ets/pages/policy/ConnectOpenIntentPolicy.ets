import { CONNECT_INTENT_SCAN } from '../state/AppShellState';

/**
 * Where the connect sheet opens, and what Back means once it is there.
 */
export class ConnectOpenIntentPolicy {
  /**
   * A signed-in phone normally opens on its account's device list, because that
   * is the connection it already has and the one most taps mean. But the entry
   * labelled 「扫描二维码连接」 means the camera and nothing else — landing it on a
   * device picker reads as the wrong screen rather than as a shortcut, and the
   * scanner it promised is then two taps further in.
   */
  static initialStep(
    openIntent: string,
    accountAuthenticated: boolean,
    remoteUrl: string,
    currentStep: string
  ): string {
    if (openIntent === CONNECT_INTENT_SCAN) {
      return 'scan';
    }
    if (accountAuthenticated) {
      return 'account';
    }
    return remoteUrl.trim().length === 0 ? 'scan' : currentStep;
  }

  /**
   * Back retraces the way in. A sheet opened straight onto the scanner has no
   * step behind it, so it leaves rather than reveals a picker the user never
   * passed through.
   */
  static backStaysInSheet(
    openIntent: string,
    currentStep: string,
    remoteUrl: string,
    showManualPairing: boolean
  ): boolean {
    return openIntent !== CONNECT_INTENT_SCAN &&
      currentStep === 'scan' &&
      remoteUrl.trim().length === 0 &&
      !showManualPairing;
  }
}
