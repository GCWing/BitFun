/*
 * Copyright (c) 2026 Huawei Device Co., Ltd.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { spawn } from 'node:child_process';

function binary(name) {
  return process.platform === 'win32' ? `${name}.exe` : name;
}

function envPath() {
  return String(process.env.DEVECO_HOME || '').trim();
}

async function isDir(file) {
  if (!file) {
    return false;
  }
  return fs.stat(file).then((info) => info.isDirectory()).catch(() => false);
}

function nodePath(home) {
  return process.platform === 'win32'
    ? path.join(home, 'tools', 'node', 'node.exe')
    : path.join(home, 'tools', 'node', 'bin', 'node');
}

function hdcPath(home) {
  return path.join(home, 'sdk', 'default', 'openharmony', 'toolchains', binary('hdc'));
}

async function exists(file) {
  return fs.access(file).then(() => true).catch(() => false);
}

async function findDevEcoHome() {
  const env = envPath();
  if (env && (await isDir(env)) && (await exists(nodePath(env)))) {
    return env;
  }
}

export function targetArgs(deviceId) {
  return deviceId ? ['-t', deviceId] : [];
}

export async function resolveHdcBinary() {
  const home = await findDevEcoHome();
  if (!home) {
    return { hdc: '', home: '', error: 'DevEco Studio path not found. Set DEVECO_HOME and retry.' };
  }

  const hdc = hdcPath(home);
  if (!(await exists(hdc))) {
    return { hdc: '', home, error: `hdc not found: ${hdc}` };
  }

  return { hdc, home, error: '' };
}

export async function resolveHdcOrThrow() {
  const resolved = await resolveHdcBinary();
  if (resolved.error) {
    throw new Error(resolved.error);
  }

  return resolved.hdc;
}

export async function runHdc(cmd) {
  return new Promise((resolve) => {
    const proc = spawn(cmd[0], cmd.slice(1), {
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    proc.stdout.setEncoding('utf8');
    proc.stderr.setEncoding('utf8');
    proc.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    proc.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    proc.on('error', (error) => {
      resolve({ stdout, stderr: error.message || stderr, exitCode: 1 });
    });
    proc.on('close', (code) => {
      resolve({ stdout, stderr, exitCode: code ?? 1 });
    });
  });
}
