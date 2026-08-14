import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const etsRoot = path.join(repoRoot, 'src/apps/mobile/harmonyos/entry/src/main/ets');
const pagesRoot = path.join(etsRoot, 'pages');

function walkEts(root) {
  const entries = fs.readdirSync(root, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkEts(entryPath));
    } else if (entry.isFile() && entry.name.endsWith('.ets')) {
      files.push(entryPath);
    }
  }
  return files;
}

function relative(file) {
  return path.relative(repoRoot, file).split(path.sep).join('/');
}

function imports(file) {
  const source = fs.readFileSync(file, 'utf8');
  const specs = [...source.matchAll(/from\s+['"]([^'"]+)['"]/g)].map((match) => match[1]);
  return specs.map((spec) => {
    if (!spec.startsWith('.')) {
      return spec;
    }
    return path.relative(etsRoot, path.resolve(path.dirname(file), spec)).split(path.sep).join('/');
  });
}

function filesUnder(root) {
  return walkEts(root).sort();
}

const allPages = filesUnder(pagesRoot);
const services = filesUnder(path.join(etsRoot, 'services'));
const components = allPages.filter((file) => file.includes(`${path.sep}pages${path.sep}components${path.sep}`));
const viewmodels = allPages.filter((file) => file.includes(`${path.sep}pages${path.sep}viewmodel${path.sep}`));

const serviceToPages = services
  .filter((file) => imports(file).some((spec) => spec === 'pages' || spec.startsWith('pages/')))
  .map(relative);
const componentToViewmodel = components
  .filter((file) => imports(file).some((spec) => spec === 'pages/viewmodel' || spec.startsWith('pages/viewmodel/')))
  .map(relative);
const viewmodelToComponents = viewmodels
  .filter((file) => imports(file).some((spec) => spec === 'pages/components' || spec.startsWith('pages/components/')))
  .map(relative);
const v1Components = allPages
  .filter((file) => /^\s*@Component\s*$/m.test(fs.readFileSync(file, 'utf8')))
  .map(relative);
const positionalActionConstructors = allPages
  .filter((file) => /export\s+class\s+\w+(?:Actions|Hooks)\b/.test(fs.readFileSync(file, 'utf8')) &&
    /\bconstructor\s*\(/.test(fs.readFileSync(file, 'utf8')))
  .map(relative);
const sharedConversationFields = [
  'sessions',
  'activeSession',
  'persistedMessages',
  'optimisticMessages',
  'activeTurnMessage',
  'hasMoreMessages',
  'timelineItems',
  'timelineRevision',
  'isBusy',
  'modelCatalog',
  'selectedModelId',
  'statusText',
  'chatInput',
  'selectedImages',
  'isVoiceListening'
];
const conversationPageStateFiles = [
  path.join(pagesRoot, 'state/GeneralChatPageState.ets'),
  path.join(pagesRoot, 'state/RemotePageState.ets')
];
const duplicatedConversationTraceFields = conversationPageStateFiles.flatMap((file) => {
  const source = fs.readFileSync(file, 'utf8');
  return sharedConversationFields
    .filter((field) => new RegExp(`@Trace\\s+${field}\\s*:`).test(source))
    .map((field) => `${relative(file)}:${field}`);
});
const appRootRuntimeFile = path.join(pagesRoot, 'runtime/AppRootRuntime.ets');
const appRootRuntimeSource = fs.readFileSync(appRootRuntimeFile, 'utf8');
const appRootRuntimeLines = appRootRuntimeSource.split(/\r?\n/).length - 1;
const appRootPresentationFile = path.join(pagesRoot, 'components/AppRootPresentation.ets');
const appRootPresentationSource = fs.readFileSync(appRootPresentationFile, 'utf8');
const appRootPresentationLines = appRootPresentationSource.split(/\r?\n/).length - 1;
const requiredPresentationFiles = [
  'components/AppRootOverlaySurfaces.ets',
  'components/ChatMessageChrome.ets',
  'components/ConnectManualPairingOverlay.ets',
  'components/ConversationRouteSurface.ets',
  'components/ToolInteractionPanels.ets',
  'components/WideConversationHost.ets',
  'components/remote/RemoteSurfaceHost.ets'
];
const missingPresentationFiles = requiredPresentationFiles
  .filter((file) => !fs.existsSync(path.join(pagesRoot, file)));
const componentLineBudgets = [
  ['components/ChatMessageBubble.ets', 1000],
  ['components/ConnectView.ets', 700],
  ['components/ToolStatusList.ets', 1120]
];
const extractedFilePreviewMethods = [
  'openFilePreview',
  'closeFilePreview',
  'refreshFilePreview',
  'openFilePreviewLink',
  'invalidateFilePreviewTarget'
].filter((method) => new RegExp(`^\\s{2}${method}\\s*\\(`, 'm').test(appRootRuntimeSource));
const extractedSettingsMethods = [
  'saveGeneralChatConfig',
  'testGeneralChatConfig',
  'validateGeneralChatConfig',
  'probeGeneralChatConfig',
  'effectiveGeneralChatApiKey',
  'applyGeneralChatConfig',
  'refreshGeneralChatModelCatalog'
].filter((method) => new RegExp(`^\\s{2}(?:private\\s+)?(?:async\\s+)?${method}\\s*\\(`, 'm')
  .test(appRootRuntimeSource));
const extractedCloudAccountMethods = [
  'persistDelegatedAccountSession',
  'loginCloudAccount',
  'restoreCloudAccountSession',
  'loadGeneralChatAccountModels',
  'applyCloudAccountSession',
  'logoutCloudAccount',
  'listCloudAccountDevices',
  'getRemotePermissionMode',
  'setRemotePermissionMode',
  'restoreCloudTarget',
  'expireCloudAccountSession',
  'handleRemoteConnectionError',
  'selectCloudAccountDevice'
].filter((method) => new RegExp(`^\\s{2}(?:private\\s+|protected\\s+)?(?:async\\s+)?${method}\\s*\\(`, 'm')
  .test(appRootRuntimeSource));
const extractedConversationMethods = [
  'isGeneralComposerRoute',
  'visibleChatInput',
  'visibleSelectedImages',
  'visibleVoiceListening',
  'setChatInputForRoute',
  'setSelectedImagesForRoute',
  'addSelectedImagesForRoute',
  'removeSelectedImageForRoute',
  'clearComposerForRoute',
  'setVoiceListeningForRoute',
  'setAllVoiceListening',
  'voiceInputSnapshot',
  'visibleChatBusy',
  'visibleStatusText',
  'setVisibleStatusText'
].filter((method) => new RegExp(`^\\s{2}(?:private\\s+)?(?:async\\s+)?${method}\\s*\\(`, 'm')
  .test(appRootRuntimeSource));
const extractedRemoteConversationMethods = [
  'sendChatMessage',
  'stopActiveTask',
  'renameActiveSession',
  'copyMessage',
  'downloadFile',
  'retryMessage',
  'approveTool',
  'rejectTool',
  'cancelTool',
  'answerQuestion',
  'resetChatTimeline',
  'syncChatTimelineFromStore',
  'startPolling',
  'currentChatPollingCursor',
  'updateChatPollingCursor',
  'applyChatSessionSnapshot',
  'hasRunningActiveTurn',
  'projectedTimelineItems',
  'syncAfterTurnEnded'
].filter((method) => new RegExp(`^\\s{2}(?:private\\s+)?(?:async\\s+)?${method}\\s*\\(`, 'm')
  .test(appRootRuntimeSource));
const extractedRemoteCreateMethods = [
  'createSession',
  'openRemoteCreateSession',
  'closeRemoteCreateSession',
  'loadRemoteCreateChoices',
  'loadRemoteCreateModelCatalog',
  'loadRemoteCreateDevices',
  'loadRemoteCreateWorkspaces',
  'toggleRemoteCreateDevices',
  'toggleRemoteCreateWorkspaces',
  'selectRemoteCreateDevice',
  'selectRemoteCreateWorkspace',
  'submitRemoteCreateSession',
  'createSessionInWorkspace',
  'openSession',
  'applyRemoteActiveSession',
  'deleteSession'
].filter((method) => new RegExp(`^\\s{2}(?:private\\s+)?(?:async\\s+)?${method}\\s*\\(`, 'm')
  .test(appRootRuntimeSource));
const extractedGeneralConversationMethods = [
  'openHomeSession',
  'openHomeSessionInPlace',
  'deleteHomeSession',
  'activeGeneralChatAsRemoteSession',
  'activeGeneralUploadedFileCount',
  'archiveHomeSession',
  'exportHomeSession',
  'openGeneralSession',
  'startGeneralChat',
  'sendVisibleChatMessage',
  'stopActiveChatTask',
  'closeActiveChat',
  'renameVisibleSession',
  'retryVisibleMessage',
  'downloadVisibleFile',
  'selectModel',
  'sendGeneralChatMessage',
  'stopGeneralChatStream',
  'startVisibleGeneralChat',
  'generalChatHomeStatusText',
  'prepareNewGeneralChat',
  'onVisibleChatInputChange',
  'visibleGeneralChatDraftId',
  'restoreGeneralChatDraft',
  'latestUserMessageText',
  'showHomeToast',
  'resetGeneralChatTimeline',
  'syncGeneralChatTimelineFromStore'
].filter((method) => new RegExp(`^\\s{2}(?:private\\s+)?(?:async\\s+)?${method}\\s*\\(`, 'm')
  .test(appRootRuntimeSource));
const extractedRemoteConnectionForwards = [
  'applyWorkspace',
  'applyRemotePairingProjection',
  'ensureRemoteAvailable',
  'setRemoteConnectionState',
  'setRemoteUrl',
  'setRemoteUserId',
  'setRemoteAuthenticatedUserId',
  'setRemoteStatusText',
  'setRemoteConnectionFailureKind',
  'setRemoteBusy',
  'setRemoteUrlInputVisible'
].filter((method) => new RegExp(`^\\s{2}(?:private\\s+)?(?:async\\s+)?${method}\\s*\\(`, 'm')
  .test(appRootRuntimeSource));
const appRootRuntimeStateGetters = [
  'remoteUrl', 'userId', 'authenticatedUserId', 'statusText', 'connectionState',
  'connectionFailureKind', 'isBusy', 'showRemoteUrlInput', 'workspaceName', 'workspacePath',
  'workspaceBranch', 'workspaceKind', 'assistantId', 'desktopName', 'desktopId', 'activeSession',
  'messages', 'pendingMessages', 'activeTurnMessage', 'timelineItems', 'hasMoreMessages'
].filter((getter) => new RegExp(`^\\s{2}get\\s+${getter}\\s*\\(`, 'm').test(appRootRuntimeSource));
const extractedOwnerForwards = [
  'currentRoute', 'isRoute', 'isGeneralChatVisible', 'pushRoute', 'replaceRoute', 'popRoute',
  'handleConversationIntent', 'pasteRemoteUrl', 'scanRemoteUrl', 'handleDetectedRemoteUrl',
  'showRecentWorkspaces', 'showAssistants', 'refreshSessions', 'loadMoreSessions', 'setSessionFilter',
  'openAddConnection', 'selectRemoteCreateModel', 'loadRecentWorkspacesInBackground',
  'loadOlderMessages', 'removeSelectedImage', 'persistVisibleGeneralChatDraft', 'stopPolling',
  'nudgeChatPolling', 'pollActiveSession', 'startHeartbeat', 'stopHeartbeat',
  'checkConnectionHealth', 'resumeRemoteActivity'
].filter((method) => new RegExp(`^\\s{2}(?:private\\s+)?(?:async\\s+)?${method}\\s*\\(`, 'm')
  .test(appRootRuntimeSource));

const expected = {
  serviceToPages: [],
  componentToViewmodel: [],
  viewmodelToComponents: [],
  v1Components: [],
  positionalActionConstructors: [],
  duplicatedConversationTraceFields: [],
  extractedFilePreviewMethods: [],
  extractedSettingsMethods: [],
  extractedCloudAccountMethods: [],
  extractedConversationMethods: [],
  extractedRemoteConversationMethods: [],
  extractedRemoteCreateMethods: [],
  extractedGeneralConversationMethods: [],
  extractedRemoteConnectionForwards: [],
  appRootRuntimeStateGetters: [],
  extractedOwnerForwards: [],
  missingPresentationFiles: []
};

function sameSet(actual, wanted) {
  return actual.length === wanted.length && actual.every((item, index) => item === wanted[index]);
}

const actual = {
  serviceToPages,
  componentToViewmodel,
  viewmodelToComponents,
  v1Components,
  positionalActionConstructors,
  duplicatedConversationTraceFields,
  extractedFilePreviewMethods,
  extractedSettingsMethods,
  extractedCloudAccountMethods,
  extractedConversationMethods,
  extractedRemoteConversationMethods,
  extractedRemoteCreateMethods,
  extractedGeneralConversationMethods,
  extractedRemoteConnectionForwards,
  appRootRuntimeStateGetters,
  extractedOwnerForwards,
  missingPresentationFiles
};
let failed = false;
for (const [name, wanted] of Object.entries(expected)) {
  if (!sameSet(actual[name], wanted)) {
    failed = true;
    console.error(`${name} mismatch`);
    console.error(`expected: ${JSON.stringify(wanted)}`);
    console.error(`actual:   ${JSON.stringify(actual[name])}`);
  }
}
if (appRootRuntimeLines > 500) {
  failed = true;
  console.error(`AppRootRuntime line budget exceeded: expected <=500, actual=${appRootRuntimeLines}`);
}
if (appRootPresentationLines > 500) {
  failed = true;
  console.error(`AppRootPresentation line budget exceeded: expected <=500, actual=${appRootPresentationLines}`);
}
for (const [file, budget] of componentLineBudgets) {
  const source = fs.readFileSync(path.join(pagesRoot, file), 'utf8');
  const lineCount = source.split(/\r?\n/).length - 1;
  if (lineCount > budget) {
    failed = true;
    console.error(`${file} line budget exceeded: expected <=${budget}, actual=${lineCount}`);
  }
}

if (failed) {
  process.exitCode = 1;
} else {
  console.log('HarmonyOS architecture contracts are satisfied.');
}
