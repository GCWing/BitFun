import { Want } from '@kit.AbilityKit';
import { Driver, ON, abilityDelegatorRegistry } from '@kit.TestKit';
import { describe, it, expect } from '@ohos/hypium';

const APP_BUNDLE: string = 'com.bitfun.app';
const APP_ABILITY: string = 'EntryAbility';

export default function deviceSmokeTest() {
  describe('BitFunDeviceSmoke', () => {
    it('opens the home surface and remote conversation entry', 0, async () => {
      const want: Want = {
        bundleName: APP_BUNDLE,
        abilityName: APP_ABILITY
      };
      await abilityDelegatorRegistry.getAbilityDelegator().startAbility(want);
      const driver = Driver.create();
      await driver.delayMs(800);

      const homeTitle = await driver.findComponent(ON.text('BitFun'));
      expect(homeTitle !== undefined).assertTrue();

      await driver.click(130, 180);
      await driver.delayMs(300);
      const remoteEntry = await driver.findComponent(ON.id('conversation-source-remote'));
      expect(remoteEntry !== undefined).assertTrue();
      await remoteEntry.click();
      await driver.delayMs(900);

      const remoteTitle = await driver.findComponent(ON.text('远程'));
      expect(remoteTitle !== undefined).assertTrue();
    });
  });
}
