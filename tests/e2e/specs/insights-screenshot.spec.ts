/**
 * Quick screenshot test for insights scene.
 */

import { browser } from '@wdio/globals';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('Insights Screenshot', () => {
  it('should take screenshot of insights scene', async () => {
    if (process.platform === 'linux') {
      return;
    }

    console.log('[Screenshot] Waiting for app to load...');
    await browser.pause(5000);

    const screenshotsDir = path.resolve(__dirname, 'reports', 'screenshots');
    if (!fs.existsSync(screenshotsDir)) {
      fs.mkdirSync(screenshotsDir, { recursive: true });
    }

    const filePath = path.join(screenshotsDir, 'insights-scene.png');
    await browser.saveScreenshot(filePath);
    console.log(`[Screenshot] Saved to: ${filePath}`);
  });
});
