/** Observable card the page owns. Model must not import pages/state. */
export interface WatchProvisionDisplay {
  ask(deviceName: string, deviceId: string): void;
  working(): void;
  requirePassword(message: string): void;
  done(message: string): void;
  fail(message: string): void;
  hide(): void;
}
