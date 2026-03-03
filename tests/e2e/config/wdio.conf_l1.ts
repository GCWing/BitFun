import type { Options } from '@wdio/types';
import { spawn, spawnSync, type ChildProcess } from 'child_process';
import * as path from 'path';
import * as os from 'os';
import * as fs from 'fs';
import * as net from 'net';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

// ESM-compatible __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

let tauriDriver: ChildProcess | null = null;
let devServer: ChildProcess | null = null;

const MSEDGEDRIVER_PATHS = [
  path.join(os.tmpdir(), 'msedgedriver.exe'),
  'C:\\Windows\\System32\\msedgedriver.exe',
  path.join(os.homedir(), 'AppData', 'Local', 'Temp', 'msedgedriver.exe'),
];

/**
 * Find msedgedriver executable
 */
function findMsEdgeDriver(): string | null {
  for (const p of MSEDGEDRIVER_PATHS) {
    if (fs.existsSync(p)) {
      return p;
    }
  }
  return null;
}

/**
 * Get the path to the tauri-driver executable
 */
function getTauriDriverPath(): string {
  const homeDir = os.homedir();
  const isWindows = process.platform === 'win32';
  const driverName = isWindows ? 'tauri-driver.exe' : 'tauri-driver';
  return path.join(homeDir, '.cargo', 'bin', driverName);
}

/** Get the path to the built Tauri application. Prefer release build; fall back to debug (requires dev server). */
function getApplicationPath(): string {
  const isWindows = process.platform === 'win32';
  const appName = isWindows ? 'bitfun-desktop.exe' : 'bitfun-desktop';
  const projectRoot = path.resolve(__dirname, '..', '..', '..');
  const releasePath = path.join(projectRoot, 'target', 'release', appName);
  if (fs.existsSync(releasePath)) {
    return releasePath;
  }
  return path.join(projectRoot, 'target', 'debug', appName);
}

/**
 * Check if tauri-driver is installed
 */
function checkTauriDriver(): boolean {
  const driverPath = getTauriDriverPath();
  return fs.existsSync(driverPath);
}

export const config: Options.Testrunner = {
  runner: 'local',
  autoCompileOpts: {
    autoCompile: true,
    tsNodeOpts: {
      transpileOnly: true,
      project: path.resolve(__dirname, '..', 'tsconfig.json'),
    },
  },

  specs: [
    '../specs/l1-ui-navigation.spec.ts',
    '../specs/l1-workspace.spec.ts',
    '../specs/l1-chat-input.spec.ts',
    '../specs/l1-navigation.spec.ts',
    '../specs/l1-file-tree.spec.ts',
    '../specs/l1-editor.spec.ts',
    '../specs/l1-terminal.spec.ts',
    '../specs/l1-git-panel.spec.ts',
    '../specs/l1-settings.spec.ts',
    '../specs/l1-session.spec.ts',
    '../specs/l1-dialog.spec.ts',
    '../specs/l1-chat.spec.ts',
  ],
  exclude: [],

  maxInstances: 1,
  capabilities: [{
    maxInstances: 1,
    'tauri:options': {
      application: getApplicationPath(),
    },
  }],

  logLevel: 'info',
  bail: 0,
  baseUrl: '',
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  services: [],
  hostname: 'localhost',
  port: 4444,
  path: '/',

  framework: 'mocha',
  reporters: ['spec'],

  mochaOpts: {
    ui: 'bdd',
    timeout: 120000,
    retries: 0,
  },

  /** Before test run: check prerequisites and start dev server. */
  onPrepare: async function () {
    console.log('Preparing L1 E2E test run...');
    
    // Check if tauri-driver is installed
    if (!checkTauriDriver()) {
      console.error('tauri-driver not found. Please install it with:');
      console.error('cargo install tauri-driver --locked');
      throw new Error('tauri-driver not installed');
    }
    console.log(`tauri-driver: ${getTauriDriverPath()}`);
    
    // Check if msedgedriver exists
    const msedgeDriverPath = findMsEdgeDriver();
    if (msedgeDriverPath) {
      console.log(`msedgedriver: ${msedgeDriverPath}`);
    } else {
      console.warn('msedgedriver not found. Will try to use PATH.');
    }
    
    // Check if the application is built
    const appPath = getApplicationPath();
    if (!fs.existsSync(appPath)) {
      console.error(`Application not found at: ${appPath}`);
      console.error('Please build the application first with:');
      console.error('npm run desktop:build');
      throw new Error('Application not built');
    }
    console.log(`application: ${appPath}`);
    
    // Check if using debug build - check if dev server is running
    if (appPath.includes('debug')) {
      console.log('Debug build detected, checking dev server...');
      
      // Check if dev server is already running on port 1422
      const isRunning = await new Promise<boolean>((resolve) => {
        const client = new net.Socket();
        client.setTimeout(2000);
        client.connect(1422, 'localhost', () => {
          client.destroy();
          resolve(true);
        });
        client.on('error', () => {
          client.destroy();
          resolve(false);
        });
        client.on('timeout', () => {
          client.destroy();
          resolve(false);
        });
      });
      
      if (isRunning) {
        console.log('Dev server is already running on port 1422');
      } else {
        console.warn('Dev server not running on port 1422');
        console.warn('Please start it with: npm run dev');
        console.warn('Continuing anyway...');
      }
    }
  },

  /** Before session: start tauri-driver. */
  beforeSession: function () {
    console.log('Starting tauri-driver...');
    
    const driverPath = getTauriDriverPath();
    const msedgeDriverPath = findMsEdgeDriver();
    const appPath = getApplicationPath();
    
    const args: string[] = [];
    
    if (msedgeDriverPath) {
      console.log(`msedgedriver: ${msedgeDriverPath}`);
      args.push('--native-driver', msedgeDriverPath);
    } else {
      console.warn('msedgedriver not found in common paths');
    }
    
    console.log(`Application: ${appPath}`);
    console.log(`Starting: ${driverPath} ${args.join(' ')}`);
    
    tauriDriver = spawn(driverPath, args, {
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    tauriDriver.stdout?.on('data', (data: Buffer) => {
      console.log(`[tauri-driver] ${data.toString().trim()}`);
    });

    tauriDriver.stderr?.on('data', (data: Buffer) => {
      console.error(`[tauri-driver] ${data.toString().trim()}`);
    });

    return new Promise<void>((resolve) => {
      setTimeout(() => {
        console.log('tauri-driver started on port 4444');
        resolve();
      }, 2000);
    });
  },

  /** After session: stop tauri-driver. */
  afterSession: function () {
    console.log('Stopping tauri-driver...');
    
    if (tauriDriver) {
      tauriDriver.kill();
      tauriDriver = null;
      console.log('tauri-driver stopped');
    }
  },

  /** After test: capture screenshot on failure. */
  afterTest: async function (test, context, { error, passed }) {
    if (!passed) {
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
      const screenshotName = `failure-${test.title.replace(/\s+/g, '_')}-${timestamp}.png`;
      
      try {
        const screenshotPath = path.resolve(__dirname, '..', 'reports', 'screenshots', screenshotName);
        await browser.saveScreenshot(screenshotPath);
        console.log(`Screenshot saved: ${screenshotName}`);
      } catch (e) {
        console.error('Failed to save screenshot:', e);
      }
    }
  },

  /** After test run: cleanup. */
  onComplete: function () {
    console.log('L1 E2E test run completed');
    if (tauriDriver) {
      tauriDriver.kill();
      tauriDriver = null;
    }
    if (devServer) {
      console.log('Stopping dev server...');
      devServer.kill();
      devServer = null;
      console.log('Dev server stopped');
    }
  },
};

export default config;
