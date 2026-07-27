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

import { promises as fs } from 'node:fs';
import path from 'node:path';
import process from 'node:process';

function binary(name: string): string {
  return process.platform === 'win32' ? `${name}.exe` : name;
}

function envPath(): string {
  return String(process.env.DEVECO_HOME || '').trim();
}

async function isDir(file: string): Promise<boolean> {
  if (!file) {
    return false;
  }
  return fs.stat(file).then((info) => info.isDirectory()).catch(() => false);
}

function nodePath(home: string): string {
  return process.platform === 'win32'
    ? path.join(home, 'tools', 'node', 'node.exe')
    : path.join(home, 'tools', 'node', 'bin', 'node');
}

function hdcPath(home: string): string {
  return path.join(home, 'sdk', 'default', 'openharmony', 'toolchains', binary('hdc'));
}

async function exists(file: string): Promise<boolean> {
  return Bun.file(file).exists();
}

async function findDevEcoHome(): Promise<string | undefined> {
  const env = envPath();
  if (env && (await isDir(env)) && (await exists(nodePath(env)))) {
    return env;
  }
  return undefined;
}

export function targetArgs(deviceId: string | undefined) {
  return deviceId ? ['-t', deviceId] : [];
}

export async function resolveHdcBinary() {
  const home = await findDevEcoHome();
  if (!home) {
    return { hdc: '', home: '', error: 'DevEco Studio path not found. Set DEVECO_HOME and retry.' };
  }

  const hdc = hdcPath(home);
  if (!(await Bun.file(hdc).exists())) {
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

export async function runHdc(cmd: string[]) {
  const proc = Bun.spawn({
    cmd,
    stdout: 'pipe',
    stderr: 'pipe',
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    proc.stdout ? Bun.readableStreamToText(proc.stdout) : Promise.resolve(''),
    proc.stderr ? Bun.readableStreamToText(proc.stderr) : Promise.resolve(''),
    proc.exited,
  ]);

  return { stdout, stderr, exitCode };
}
