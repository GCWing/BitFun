// Source boundary rule entrypoint. Keep detailed rules in focused modules.

export { facadeOnlyFiles } from './source/facade-rules.mjs';
export {
  forbiddenContentRules,
  forbiddenContentUnderRules,
} from './source/forbidden-rules.mjs';
export { requiredContentRules } from './source/required-rules.mjs';
