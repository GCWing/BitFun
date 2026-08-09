import transportAndGeneralChatUnitTest from './TransportAndGeneralChatUnit.test';
import conversationStateUnitTest from './ConversationStateUnit.test';
import lifecycleUnitTest from './LifecycleUnit.test';
import appRootLifecycleUnitTest from './AppRootLifecycleUnit.test';
import remoteControllersUnitTest from './RemoteControllersUnit.test';
import appRootRuntimeStartupUnitTest from './AppRootRuntimeStartupUnit.test';

export default function localUnitTest() {
  transportAndGeneralChatUnitTest();
  conversationStateUnitTest();
  lifecycleUnitTest();
  appRootLifecycleUnitTest();
  remoteControllersUnitTest();
  appRootRuntimeStartupUnitTest();
}
