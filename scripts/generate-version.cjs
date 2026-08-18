#!/usr/bin/env node

/**
 * Version info generation script
 * Generates version file with version, build date, Git info at build time
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const {
  printSuccess,
  printInfo,
  printWarning,
  colors,
  colorize,
} = require('./console-style.cjs');

const packageJsonPath = path.resolve(__dirname, '../package.json');
const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));

function parseBuildEnv(args) {
  const index = args.indexOf('--build-env');
  const buildEnv = index >= 0 ? args[index + 1] : undefined;
  if (!['development', 'production', 'preview'].includes(buildEnv)) {
    throw new Error('Expected --build-env development|production|preview');
  }
  return buildEnv;
}

function readArg(args, name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

function getGitInfo() {
  try {
    const gitCommitFull = execSync('git rev-parse HEAD', { encoding: 'utf-8' }).trim();
    const gitCommit = gitCommitFull.substring(0, 7);
    const gitBranch = execSync('git rev-parse --abbrev-ref HEAD', { encoding: 'utf-8' }).trim();
    
    return {
      gitCommit,
      gitCommitFull,
      gitBranch
    };
  } catch (error) {
    printWarning('Could not get Git info (may not be a Git repo)');
    return {
      gitCommit: undefined,
      gitCommitFull: undefined,
      gitBranch: undefined
    };
  }
}

function generateVersionInfo(buildEnv) {
  const gitInfo = getGitInfo();
  const buildDate = new Date().toISOString();
  const buildTimestamp = Date.now();
  const isDev = buildEnv === 'development';
  const releaseChannel = process.env.BITFUN_RELEASE_CHANNEL || 'stable';
  if (!['stable', 'beta', 'nightly'].includes(releaseChannel)) {
    throw new Error(`Unsupported BITFUN_RELEASE_CHANNEL: ${releaseChannel}`);
  }
  
  const versionInfo = {
    name: packageJson.name === 'BitFun' ? 'BitFun' : packageJson.name,
    version: packageJson.version,
    buildDate,
    buildTimestamp,
    buildEnv,
    releaseChannel,
    isDev,
    ...gitInfo
  };
  
  return versionInfo;
}

function saveVersionInfoToJson(versionInfo, outputRoot) {
  const outputPath = path.resolve(outputRoot, 'src/web-ui/public/version.json');
  
  const dir = path.dirname(outputPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  
  fs.writeFileSync(
    outputPath,
    JSON.stringify(versionInfo, null, 2),
    'utf-8'
  );
}

function saveVersionInfoToTS(versionInfo, outputRoot) {
  const outputPath = path.resolve(outputRoot, 'src/web-ui/src/generated/version.ts');
  
  const dir = path.dirname(outputPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  
  const content = `/**
 * Auto-generated version info. Do not edit.
 * Generated: ${new Date().toISOString()}
 */

import type { VersionInfo } from '../shared/types/version';

export const VERSION_INFO: VersionInfo = ${JSON.stringify(versionInfo, null, 2)};
`;
  
  fs.writeFileSync(outputPath, content, 'utf-8');
}

function generateHtmlInjectionScript(versionInfo) {
  return `<script>
  // Version info injected at build time
  window.__VERSION_INFO__ = ${JSON.stringify(versionInfo)};
</script>`;
}

function main() {
  const args = process.argv.slice(2);
  const buildEnv = parseBuildEnv(args);
  const outputRoot = path.resolve(readArg(args, '--output-root') || path.resolve(__dirname, '..'));
  const versionInfo = generateVersionInfo(buildEnv);
  
  saveVersionInfoToJson(versionInfo, outputRoot);
  saveVersionInfoToTS(versionInfo, outputRoot);
  
  const htmlScript = generateHtmlInjectionScript(versionInfo);
  const htmlScriptPath = path.resolve(outputRoot, 'src/web-ui/src/generated/version-injection.html');
  
  const htmlDir = path.dirname(htmlScriptPath);
  if (!fs.existsSync(htmlDir)) {
    fs.mkdirSync(htmlDir, { recursive: true });
  }
  
  fs.writeFileSync(htmlScriptPath, htmlScript, 'utf-8');
  
  const gitStr = versionInfo.gitCommit ? ` ${versionInfo.gitBranch}@${versionInfo.gitCommit}` : '';
  printSuccess(`${versionInfo.name} v${versionInfo.version}${gitStr}`);
}

try {
  main();
} catch (err) {
  printWarning('Version info generation failed: ' + (err.message || err));
  process.exit(1);
}

