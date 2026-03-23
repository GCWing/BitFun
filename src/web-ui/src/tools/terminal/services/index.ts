/**
 * Terminal service exports.
 */

export { TerminalService, getTerminalService } from './TerminalService';
export {
  deleteManualTerminalProfile,
  generateManualTerminalProfileId,
  getManualTerminalProfileById,
  getManualTerminalProfileBySessionId,
  listManualTerminalProfiles,
  loadManualTerminalProfiles,
  saveManualTerminalProfiles,
  upsertManualTerminalProfile,
  type ManualTerminalProfile,
  type ManualTerminalProfileInput,
  type ManualTerminalProfilesState,
} from './manualTerminalProfileService';

export { 
  terminalActionManager,
  registerTerminalActions,
  unregisterTerminalActions,
  type TerminalActionHandler 
} from './TerminalActionManager';
