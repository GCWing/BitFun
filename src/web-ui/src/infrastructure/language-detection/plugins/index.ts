 

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('LanguageDetection');

 
export async function registerBuiltinPlugins(): Promise<void> {
  try {
    
    // await registerWebFrameworkPlugins();
    // await registerPythonFrameworkPlugins();
    
    log.debug('Builtin plugins registered');
  } catch (error) {
    log.error('Failed to register plugins', error);
  }
}
