import type { GlobalSearchProvider } from '../types';
import { actionSearchProvider } from './actionSearchProvider';
import { entitySearchProvider } from './entitySearchProvider';
import { fileSearchProvider } from './fileSearchProvider';
import { sessionContentSearchProvider } from './sessionContentSearchProvider';
import { settingsSearchProvider } from './settingsSearchProvider';

export const GLOBAL_SEARCH_PROVIDERS: readonly GlobalSearchProvider[] = [
  actionSearchProvider,
  entitySearchProvider,
  settingsSearchProvider,
  sessionContentSearchProvider,
  fileSearchProvider,
];

export {
  actionSearchProvider,
  entitySearchProvider,
  fileSearchProvider,
  sessionContentSearchProvider,
  settingsSearchProvider,
};
