import abilityTest from './Ability.test';
import deviceSmokeTest from './DeviceSmoke.test';
import nativeCryptoLoadTest from './NativeCryptoLoad.test';

export default function testsuite() {
  abilityTest();
  deviceSmokeTest();
  nativeCryptoLoadTest();
}
