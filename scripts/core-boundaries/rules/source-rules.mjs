// Source boundary rule entrypoint. Keep detailed rules in focused modules.

export { checkLocalCustomizationSymbols, localCustomizationSymbols } from './local-customization-symbols.mjs';
export { facadeOnlyFiles } from './source/facade-rules.mjs';
export {
  forbiddenContentRules,
  forbiddenContentUnderRules,
  rustWebUiSourceBoundaryRule,
} from './source/forbidden-rules.mjs';
export {
  agentRuntimeRootPublicModules,
  publicApiAllowlistRules,
  publicApiContractSlices,
} from './source/public-api-rules.mjs';
export { requiredContentRules } from './source/required-rules.mjs';
