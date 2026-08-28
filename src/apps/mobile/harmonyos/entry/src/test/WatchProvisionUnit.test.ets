import { describe, expect, it } from '@ohos/hypium';
import { RemoteI18n } from '../main/ets/i18n/RemoteI18n';
import {
  WatchProvisionController,
  WatchProvisionOutcome,
  WatchProvisionPort
} from '../main/ets/services/WatchProvisionController';
import { WatchProvisionDisplay } from '../main/ets/services/WatchProvisionDisplay';

class FakeWatchProvisionDisplay implements WatchProvisionDisplay {
  phase: string = 'hidden';
  message: string = '';

  ask(_deviceName: string, _deviceId: string): void {
    this.phase = 'asking';
    this.message = '';
  }

  working(): void {
    this.phase = 'working';
    this.message = '';
  }

  requirePassword(message: string): void {
    this.phase = 'password';
    this.message = message;
  }

  done(message: string): void {
    this.phase = 'done';
    this.message = message;
  }

  fail(message: string): void {
    this.phase = 'failed';
    this.message = message;
  }

  hide(): void {
    this.phase = 'hidden';
    this.message = '';
  }
}

function watchRequest(): string {
  return JSON.stringify({
    v: 1,
    request_id: '12345678-1234-4234-8234-123456789abc',
    device_id: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    device_name: 'HarmonyOS Watch',
    public_key: 'test-public-key',
    created_ms: Date.now()
  });
}

function passwordRequired(): WatchProvisionOutcome {
  return {
    ok: false,
    passwordRequired: true,
    relayUrl: '', token: '', userId: '', masterKeyBase64: '', deviceId: '', failure: '',
    desktopReported: false
  };
}

export default function watchProvisionUnitTest() {
  describe('WatchProvisionController', () => {
    it('keeps the watch request pending while account confirmation is needed', 0, async () => {
      const display = new FakeWatchProvisionDisplay();
      const passwords: string[] = [];
      const port: WatchProvisionPort = {
        canProvision: (): boolean => true,
        provision: async (
          _deviceId: string,
          _deviceName: string,
          _requestId: string,
          password: string
        ): Promise<WatchProvisionOutcome> => {
          passwords.push(password);
          return passwordRequired();
        }
      };
      const controller = new WatchProvisionController(display, port);
      controller.onRequestPayload(watchRequest());

      await controller.approve();
      expect(display.phase).assertEqual('password');
      await controller.approve('test-password');

      expect(passwords.length).assertEqual(2);
      expect(passwords[0]).assertEqual('');
      expect(passwords[1]).assertEqual('test-password');
      expect(display.phase).assertEqual('password');
    });

    it('returns to password confirmation when the compatibility login fails', 0, async () => {
      const display = new FakeWatchProvisionDisplay();
      const port: WatchProvisionPort = {
        canProvision: (): boolean => true,
        provision: async (
          _deviceId: string,
          _deviceName: string,
          _requestId: string,
          _password: string
        ): Promise<WatchProvisionOutcome> => {
          throw new Error('login rejected');
        }
      };
      const controller = new WatchProvisionController(display, port);
      controller.onRequestPayload(watchRequest());

      await controller.approve('wrong-password');

      expect(display.phase).assertEqual('password');
      expect(display.message.length > 0).assertTrue();
    });

    it('does not report a phone account failure as a desktop outage', 0, async () => {
      const display = new FakeWatchProvisionDisplay();
      const port: WatchProvisionPort = {
        canProvision: (): boolean => true,
        provision: async (
          _deviceId: string,
          _deviceName: string,
          _requestId: string,
          _password: string
        ): Promise<WatchProvisionOutcome> => {
          throw new Error('relay unavailable');
        }
      };
      const controller = new WatchProvisionController(display, port);
      controller.onRequestPayload(watchRequest());

      await controller.approve();

      expect(display.phase).assertEqual('failed');
      expect(display.message).assertEqual(RemoteI18n.t('watchProvision.errors.accountUnavailable'));
    });
  });
}
