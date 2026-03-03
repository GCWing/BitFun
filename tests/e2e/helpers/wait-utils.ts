/**
 * Wait utilities for E2E tests.
 * Contains commonly used wait functions for element stability and interactions.
 */
import { browser, $ } from '@wdio/globals';
import { environmentSettings } from '../config/capabilities';

/**
 * Wait for an element to become stable (no position/size changes).
 * Used to ensure animations have completed before interacting with elements.
 * 
 * @param selector - CSS selector for the element
 * @param stableTime - Time in ms the element must remain stable (default: 500ms)
 * @param timeout - Maximum time to wait (default: from environmentSettings)
 */
export async function waitForElementStable(
  selector: string,
  stableTime: number = 500,
  timeout: number = environmentSettings.defaultTimeout
): Promise<void> {
  const startTime = Date.now();
  let lastRect: { x: number; y: number; width: number; height: number } | null = null;
  let stableStartTime: number | null = null;

  await browser.waitUntil(
    async () => {
      const element = await $(selector);
      if (!(await element.isDisplayed())) {
        return false;
      }

      const rect = await element.getLocation();
      const size = await element.getSize();
      const currentRect = {
        x: rect.x,
        y: rect.y,
        width: size.width,
        height: size.height,
      };

      if (lastRect && 
          lastRect.x === currentRect.x &&
          lastRect.y === currentRect.y &&
          lastRect.width === currentRect.width &&
          lastRect.height === currentRect.height) {
        if (!stableStartTime) {
          stableStartTime = Date.now();
        }
        return Date.now() - stableStartTime >= stableTime;
      } else {
        lastRect = currentRect;
        stableStartTime = null;
        return false;
      }
    },
    {
      timeout,
      timeoutMsg: `Element ${selector} did not stabilize within ${timeout}ms`,
      interval: 100,
    }
  );
}
