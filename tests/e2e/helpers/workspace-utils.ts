/**
 * Workspace utilities for E2E tests
 */

import { StartupPage } from '../page-objects/StartupPage';
import { browser } from '@wdio/globals';

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

  console.log('[WorkspaceUtils] No workspace open - attempting to open test workspace');

  // Try to open a recent workspace first
  const openedRecent = await startupPage.openRecentWorkspace(0);

  if (openedRecent) {
    console.log('[WorkspaceUtils] Recent workspace opened successfully');
    await browser.pause(2000); // Wait for workspace to fully load
    return true;
  }

  // If no recent workspace, try to open current project directory
  // Use environment variable or default to relative path
  const testWorkspacePath = process.env.E2E_TEST_WORKSPACE || process.cwd();
  console.log('[WorkspaceUtils] Opening test workspace:', testWorkspacePath);

  try {
    await startupPage.openWorkspaceByPath(testWorkspacePath);
    console.log('[WorkspaceUtils] Test workspace opened successfully');
    await browser.pause(2000); // Wait for workspace to fully load

    // After opening workspace, we might still be on welcome scene
    // Need to create a new session to get to the chat interface
    await createNewSession();

    return true;
  } catch (error) {
    console.error('[WorkspaceUtils] Failed to open test workspace:', error);
    return false;
  }
}

/**
 * Create a new code session after workspace is opened
 */
async function createNewSession(): Promise<void> {
  try {
    console.log('[WorkspaceUtils] Creating new session...');

    // Look for "New Code Session" button on welcome scene
    const newSessionSelectors = [
      'button:has-text("New Code Session")',
      '.welcome-scene__session-btn',
      'button[class*="session-btn"]',
    ];

    for (const selector of newSessionSelectors) {
      try {
        const button = await browser.$(selector);
        const exists = await button.isExisting();

        if (exists) {
          console.log(`[WorkspaceUtils] Found new session button: ${selector}`);
          await button.click();
          await browser.pause(1500); // Wait for session to be created
          console.log('[WorkspaceUtils] New session created');
          return;
        }
      } catch (e) {
        // Try next selector
      }
    }

    console.log('[WorkspaceUtils] Could not find new session button, may already be in session');
  } catch (error) {
    console.error('[WorkspaceUtils] Failed to create new session:', error);
  }
}
