/**
 * Workspace utilities for E2E tests
 */

import { StartupPage } from '../page-objects/StartupPage';
import { ensureCodeSessionOpen, openWorkspace } from './workspace-helper';

/**
 * Ensure a workspace is open for testing.
 * If no workspace is open, attempts to open one automatically.
 *
 * @param startupPage - The StartupPage instance
 * @returns true if workspace is open, false otherwise
 */
export async function ensureWorkspaceOpen(startupPage: StartupPage): Promise<boolean> {
  const startupVisible = await startupPage.isVisible();

  if (!startupVisible) {
    // Workspace is already open
    return true;
  }

  const testWorkspacePath = process.env.E2E_TEST_WORKSPACE || process.cwd();
  console.log('[WorkspaceUtils] Opening test workspace through frontend state:', testWorkspacePath);

  try {
    const opened = await openWorkspace(testWorkspacePath);
    if (!opened) {
      return false;
    }
    await ensureCodeSessionOpen();
    console.log('[WorkspaceUtils] Test workspace opened successfully');
    return true;
  } catch (error) {
    console.error('[WorkspaceUtils] Failed to open test workspace:', error);
    return false;
  }
}
