import { CONNECT_INTENT_SCAN } from '../state/AppShellState';

export class ConnectSheetStep {
  static readonly Intro: string = 'intro';
  static readonly Scan: string = 'scan';
  static readonly Account: string = 'account';
}

/**
 * Where the connect sheet should open, and which step it may keep showing.
 *
 * The camera is a named action, not the default: an unsigned-in first open
 * has to explain that the pair code comes from the desktop. A signed-in phone
 * already has an account device list. Only an entry that promised the scanner
 * may land on it.
 */
export class ConnectSheetLandingPolicy {
  static initialStep(openIntent: string, accountAuthenticated: boolean): string {
    if (openIntent === CONNECT_INTENT_SCAN) {
      return ConnectSheetStep.Scan;
    }
    if (accountAuthenticated) {
      return ConnectSheetStep.Account;
    }
    return ConnectSheetStep.Intro;
  }

  static visibleStep(pairingStep: string, accountAuthenticated: boolean): string {
    if (pairingStep === ConnectSheetStep.Account && accountAuthenticated) {
      return ConnectSheetStep.Account;
    }
    if (pairingStep === ConnectSheetStep.Scan) {
      return ConnectSheetStep.Scan;
    }
    return ConnectSheetStep.Intro;
  }
}

export class ConnectSheetStatusPolicy {
  static isConnectError(
    connectionState: string,
    isBusy: boolean,
    isConnected: boolean,
    statusText: string,
    failedLabel: string,
    missingParams: string,
    urlRequired: string
  ): boolean {
    if (connectionState === 'failed') {
      return true;
    }
    if (isBusy || isConnected) {
      return false;
    }
    const text = statusText.toLowerCase();
    return text.indexOf('http') >= 0 ||
      text.indexOf('error') >= 0 ||
      text.indexOf('failed') >= 0 ||
      text.indexOf(failedLabel.toLowerCase()) >= 0 ||
      statusText === missingParams ||
      statusText === urlRequired ||
      text.indexOf('rejected') >= 0;
  }

  static failureHintKey(kind: string): string {
    if (kind === 'expired_room') {
      return 'connect.hintExpiredRoom';
    }
    if (kind === 'network') {
      return 'connect.hintNetwork';
    }
    if (kind === 'protected_user') {
      return 'connect.hintProtectedUser';
    }
    if (kind === 'invalid_link') {
      return 'connect.hintInvalidLink';
    }
    return '';
  }
}
