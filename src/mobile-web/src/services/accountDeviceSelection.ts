export interface AccountDevice {
  device_id: string;
  device_name: string;
  online: boolean;
}

/** Device availability is not an authentication condition. A scanned target
 * must never silently fall back to an unrelated device on the same account. */
export function selectAccountDevice(
  devices: readonly AccountDevice[],
  controllerDeviceId: string | null,
  preferredDeviceId?: string | null,
): AccountDevice | null {
  const preferred = preferredDeviceId?.trim();
  return devices.find((device) => device.device_id !== controllerDeviceId
    && device.online && (!preferred || device.device_id === preferred)) ?? null;
}
