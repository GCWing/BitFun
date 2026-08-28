import { WatchProvisionProtocol } from '../../services/WatchProvisionProtocol';
import { WatchProvisionDisplay } from '../../services/WatchProvisionDisplay';

export enum WatchProvisionPhase {
  /** No watch is waiting; the card is not on screen. */
  Hidden = 'hidden',
  /** A request arrived and its owner has not answered yet. */
  Asking = 'asking',
  /** The relay needs a normal account login to add this watch. */
  Password = 'password',
  /** Approved; the phone is obtaining and handing off the credential. */
  Working = 'working',
  Done = 'done',
  Failed = 'failed'
}

@ObservedV2
export class WatchProvisionState implements WatchProvisionDisplay {
  @Trace phase: WatchProvisionPhase = WatchProvisionPhase.Hidden;
  @Trace deviceName: string = '';
  @Trace deviceIdLabel: string = '';
  @Trace message: string = '';

  ask(deviceName: string, deviceId: string): void {
    this.phase = WatchProvisionPhase.Asking;
    this.deviceName = deviceName;
    this.deviceIdLabel = WatchProvisionProtocol.shortDeviceId(deviceId);
    this.message = '';
  }

  working(): void {
    this.phase = WatchProvisionPhase.Working;
    this.message = '';
  }

  requirePassword(message: string): void {
    this.phase = WatchProvisionPhase.Password;
    this.message = message;
  }

  done(message: string): void {
    this.phase = WatchProvisionPhase.Done;
    this.message = message;
  }

  fail(message: string): void {
    this.phase = WatchProvisionPhase.Failed;
    this.message = message;
  }

  hide(): void {
    this.phase = WatchProvisionPhase.Hidden;
    this.deviceName = '';
    this.deviceIdLabel = '';
    this.message = '';
  }

  visible(): boolean {
    return this.phase !== WatchProvisionPhase.Hidden;
  }

  /** The card is only dismissible when nothing is in flight behind it. */
  dismissible(): boolean {
    return this.phase !== WatchProvisionPhase.Working;
  }
}
