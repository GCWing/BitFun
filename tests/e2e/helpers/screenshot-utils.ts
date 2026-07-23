/**
 * Screenshot and visual comparison utilities.
 */
import { browser, $ } from '@wdio/globals';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

// ESM-compatible __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

interface ScreenshotOptions {
  directory?: string;
  includeTimestamp?: boolean;
  prefix?: string;
}

function generateScreenshotName(
  baseName: string,
  options: ScreenshotOptions = {}
): string {
  const { includeTimestamp = true, prefix = '' } = options;
  
  let fileName = prefix ? `${prefix}-${baseName}` : baseName;
  
  if (includeTimestamp) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    fileName = `${fileName}-${timestamp}`;
  }
  
  return `${fileName}.png`;
}

function ensureDirectoryExists(dirPath: string): void {
  if (!fs.existsSync(dirPath)) {
    fs.mkdirSync(dirPath, { recursive: true });
  }
}

function screenshotsSupported(): boolean {
  return process.platform !== 'linux';
}

export async function saveScreenshot(
  name: string,
  options: ScreenshotOptions = {}
): Promise<string> {
  const directory = options.directory || path.resolve(__dirname, '..', 'reports', 'screenshots');
  ensureDirectoryExists(directory);
  
  const fileName = generateScreenshotName(name, options);
  const filePath = path.join(directory, fileName);

  if (!screenshotsSupported()) {
    console.warn(`Skipping screenshot on ${process.platform}: ${filePath}`);
    return filePath;
  }
  
  await browser.saveScreenshot(filePath);
  console.log(`Screenshot saved: ${filePath}`);
  
  return filePath;
}

export async function saveElementScreenshot(
  selector: string,
  name: string,
  options: ScreenshotOptions = {}
): Promise<string> {
  const directory = options.directory || path.resolve(__dirname, '..', 'reports', 'screenshots');
  ensureDirectoryExists(directory);
  
  const fileName = generateScreenshotName(name, options);
  const filePath = path.join(directory, fileName);

  if (!screenshotsSupported()) {
    console.warn(`Skipping element screenshot on ${process.platform}: ${filePath}`);
    return filePath;
  }
  
  const element = await $(selector);
  await element.saveScreenshot(filePath);
  console.log(`Element screenshot saved: ${filePath}`);
  
  return filePath;
}

export async function saveFailureScreenshot(
  testName: string,
  error?: Error
): Promise<string> {
  const fileName = `failure-${testName.replace(/\s+/g, '_')}`;
  const filePath = await saveScreenshot(fileName, {
    prefix: 'FAIL',
    includeTimestamp: true,
  });
  if (error) {
    const errorFilePath = filePath.replace('.png', '.txt');
    const errorContent = `Test: ${testName}\nError: ${error.message}\nStack: ${error.stack}`;
    fs.writeFileSync(errorFilePath, errorContent);
  }
  
  return filePath;
}

export async function saveStepScreenshot(stepName: string): Promise<string> {
  return saveScreenshot(stepName, {
    prefix: 'step',
    includeTimestamp: true,
  });
}

export async function compareScreenshots(
  baselinePath: string,
  currentPath: string
): Promise<{ match: boolean; diffPercentage: number }> {
  if (!fs.existsSync(baselinePath)) {
    console.warn(`Baseline image not found: ${baselinePath}`);
    return { match: false, diffPercentage: 100 };
  }
  
  if (!fs.existsSync(currentPath)) {
    console.warn(`Current image not found: ${currentPath}`);
    return { match: false, diffPercentage: 100 };
  }
  
  const baselineSize = fs.statSync(baselinePath).size;
  const currentSize = fs.statSync(currentPath).size;
  const sizeDiff = Math.abs(baselineSize - currentSize);
  const diffPercentage = (sizeDiff / baselineSize) * 100;
  const match = diffPercentage < 1;
  
  return { match, diffPercentage };
}

export async function createBaseline(
  name: string,
  selector?: string
): Promise<string> {
  const baselineDir = path.resolve(__dirname, '..', 'baselines');
  ensureDirectoryExists(baselineDir);
  
  const fileName = `${name}.png`;
  const filePath = path.join(baselineDir, fileName);
  
  if (selector) {
    if (!screenshotsSupported()) {
      console.warn(`Skipping baseline screenshot on ${process.platform}: ${filePath}`);
      return filePath;
    }

    const element = await $(selector);
    await element.saveScreenshot(filePath);
  } else {
    if (!screenshotsSupported()) {
      console.warn(`Skipping baseline screenshot on ${process.platform}: ${filePath}`);
      return filePath;
    }

    await browser.saveScreenshot(filePath);
  }
  
  console.log(`Baseline created: ${filePath}`);
  return filePath;
}

export function getBaselinePath(name: string): string {
  return path.resolve(__dirname, '..', 'baselines', `${name}.png`);
}

export function cleanupScreenshots(
  directory: string,
  maxAgeDays: number = 7
): void {
  if (!fs.existsSync(directory)) {
    return;
  }
  
  const now = Date.now();
  const maxAge = maxAgeDays * 24 * 60 * 60 * 1000;
  
  const files = fs.readdirSync(directory);
  
  for (const file of files) {
    const filePath = path.join(directory, file);
    const stats = fs.statSync(filePath);
    
    if (now - stats.mtimeMs > maxAge) {
      fs.unlinkSync(filePath);
      console.log(`Deleted old screenshot: ${file}`);
    }
  }
}

export default {
  saveScreenshot,
  saveElementScreenshot,
  saveFailureScreenshot,
  saveStepScreenshot,
  compareScreenshots,
  createBaseline,
  getBaselinePath,
  cleanupScreenshots,
};
