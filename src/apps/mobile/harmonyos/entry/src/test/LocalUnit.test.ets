import transportAndGeneralChatUnitTest from './TransportAndGeneralChatUnit.test';
import conversationStateUnitTest from './ConversationStateUnit.test';
import conversationPresentationUnitTest from './ConversationPresentationUnit.test';
import lifecycleUnitTest from './LifecycleUnit.test';
import appRootLifecycleUnitTest from './AppRootLifecycleUnit.test';
import remoteControllersUnitTest from './RemoteControllersUnit.test';
import appRootRuntimeStartupUnitTest from './AppRootRuntimeStartupUnit.test';

export default function localUnitTest() {
  transportAndGeneralChatUnitTest();
  conversationStateUnitTest();
  conversationPresentationUnitTest();
  lifecycleUnitTest();
  appRootLifecycleUnitTest();
  remoteControllersUnitTest();
  appRootRuntimeStartupUnitTest();
}
