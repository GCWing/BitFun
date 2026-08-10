import localUnitTest from './LocalUnit.test';
import architectureUnitTest from './ArchitectureUnit.test';

export default function testsuite() {
  localUnitTest();
  architectureUnitTest();
}
