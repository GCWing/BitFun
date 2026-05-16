export function buildMiniAppCustomizationPrompt(params: {
  appId: string;
  appName: string;
  draftId: string;
  draftRoot: string;
  userRequest: string;
}): string {
  return [
    'You are customizing a BitFun MiniApp draft.',
    `App: ${params.appName} (${params.appId})`,
    `Draft id: ${params.draftId}`,
    `Draft root: ${params.draftRoot}`,
    '',
    'Edit only files under the draft root.',
    'Do not edit the active app directory.',
    'Do not add permissions unless the user request truly needs them.',
    'If new fs, shell, net, node, npm, or ai permissions are needed, explain why before changing them.',
    'After editing source files, tell the user to refresh the draft preview before applying.',
    '',
    'User request:',
    params.userRequest.trim(),
  ].join('\n');
}
