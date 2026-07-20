/**
 * One-click self-hosted relay deploy (Desktop SSH wizard).
 *
 * Invariants and ownership: see `./README.md`. Do not rewire entry points to
 * open an external README instead of `RelayDeployWizard`.
 */
export { RelayDeployWizard, default } from './RelayDeployWizard';
export type { RelayDeployResult } from './RelayDeployWizard';
export { relayDeployApi } from './relayDeployApi';
export type {
  RelayPreflight,
  RelayDeployTask,
  RelayTaskStatus,
  RelayTaskPoll,
  RelayVerifyResult,
} from './relayDeployApi';
