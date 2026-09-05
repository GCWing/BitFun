import { Want } from '@kit.AbilityKit';
import { Driver, ON, abilityDelegatorRegistry } from '@kit.TestKit';
import { describe, it, expect } from '@ohos/hypium';
import { HarmonyUpgradeIdentityContract } from '../../../main/ets/services/HarmonyUpgradeIdentityContract';

const APP_BUNDLE: string = HarmonyUpgradeIdentityContract.APP_BUNDLE;
const APP_ABILITY: string = 'EntryAbility';

export default function deviceSmokeTest() {
  describe('OpenBitFunDeviceSmoke', () => {
    it('opens the home surface and remote conversation entry', 0, async () => {
      const want: Want = {
        bundleName: APP_BUNDLE,
        abilityName: APP_ABILITY
      };
      await abilityDelegatorRegistry.getAbilityDelegator().startAbility(want);
      const driver = Driver.create();
      await driver.delayMs(800);

      const homeTitle = await driver.findComponent(ON.text('OpenBitFun'));
      expect(homeTitle !== undefined).assertTrue();

      await driver.click(130, 180);
      await driver.delayMs(300);
      // The sidebar carries conversations and workspaces in one scroll now, so
      // the workspace section is present without a source switch ahead of it.
      const workspaceSection = await driver.findComponent(ON.id('sidebar-workspace-section'));
      expect(workspaceSection !== undefined).assertTrue();
    });
  });
}
