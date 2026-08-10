import abilityTest from './Ability.test';
import deviceSmokeTest from './DeviceSmoke.test';

export default function testsuite() {
  abilityTest();
  deviceSmokeTest();
}
