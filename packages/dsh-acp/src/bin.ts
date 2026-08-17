#!/usr/bin/env node
/**
 * Boot an IDE-facing ACP stdio server from `cordis.yml`; usage is
 * `dsh-acp-ide [--config path]`, defaulting to `./cordis.yml`. Shared env
 * loading, Loader guards, snapshot config selection, and settled-tree boot live
 * in dsh-app-boot. Stdout is reserved for JSON-RPC, so diagnostics go only to
 * stderr.
 *
 * Forked from `@deepseek-ai/dsh-acp-demo/bin`
 * (deepseek-harness/packages/examples/acp-demo/src/bin.ts, MIT).
 * Copyright (c) DeepSeek. See ../NOTICE.md.
 *
 * @module @bitfun/dsh-acp/bin
 */

import { parseArgs } from 'node:util'
import { boot, installFailLoud, loadEnv, resolveConfigPath } from '@deepseek-ai/dsh-app-boot'

const NAME = 'dsh-acp-ide'

installFailLoud(NAME)
const snapshotMode = process.env['DSH_SNAPSHOT']
if (snapshotMode !== 'replay') loadEnv(NAME)
const { values } = parseArgs({
  args: process.argv.slice(2),
  options: { config: { type: 'string', short: 'c' } },
  strict: true,
})
await boot(NAME, resolveConfigPath(values.config ?? './cordis.yml', snapshotMode))
// Shutdown on stdin close belongs to the app plugin, not to this script: the
// same composition also boots under `dsh --profile`, which runs no bin at all.
