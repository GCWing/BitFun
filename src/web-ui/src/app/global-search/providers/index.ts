import type { GlobalSearchProvider } from '../types';
import { actionSearchProvider } from './actionSearchProvider';
import { entitySearchProvider } from './entitySearchProvider';
import { fileSearchProvider } from './fileSearchProvider';
import { interactiveCapabilitySearchProvider } from './interactiveCapabilitySearchProvider';
import { sessionContentSearchProvider } from './sessionContentSearchProvider';

export const GLOBAL_SEARCH_PROVIDERS: readonly GlobalSearchProvider[] = [
  actionSearchProvider,
  entitySearchProvider,
  interactiveCapabilitySearchProvider,
  sessionContentSearchProvider,
  fileSearchProvider,
];

export {
  actionSearchProvider,
  entitySearchProvider,
  fileSearchProvider,
  interactiveCapabilitySearchProvider,
  sessionContentSearchProvider,
};
