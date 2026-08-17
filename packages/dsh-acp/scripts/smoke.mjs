#!/usr/bin/env node
/**
 * Boot smoke check: spawn the adapter exactly as an IDE would, complete the ACP
 * handshake, open a session, and report what came back.
 *
 * This is the cheap half of end-to-end — it contacts no provider, because one
 * is only reached on `session/prompt`. Pass `--prompt "<text>"` to also drive
 * one real turn and print every session update; that half needs a DeepSeek key
 * the harness can resolve, which is any of `DEEPSEEK_API_KEY` in this
 * environment, `$DSH_HOME/.credentials.yaml` (what dsh's Models page writes),
 * or a `.env` fallback. This script asserts none of them: the credential chain
 * belongs to the composition, and a missing key surfaces as a failed turn.
 *
 * Permission requests are answered `allow-once`, or `reject-once` with
 * `--reject` — enough to exercise both branches of the approval path.
 *
 * `--cancel-after <ms>` interrupts the turn mid-flight and then sends a second
 * prompt, so a session that cancels but cannot continue fails loudly.
 *
 * `--mode <id>` sets the session's mode config option before that prompt,
 * switching the agent preset. It stays in the credential-free half: a mode
 * switch is answered by the bridge and never reaches the provider.
 *
 * `--load <sessionId>` reopens a stored session instead of starting one, which
 * is what an IDE does after a restart: the run then prints the replayed history
 * and the mode the session comes back locked to. The id is a dsh session id —
 * a directory name under `$DSH_HOME/acp-sessions/<project>/` — and `--cwd` must
 * name the workspace it was created in.
 *
 * Usage: `node scripts/smoke.mjs [--mode code] [--model deepseek-official/deepseek-v4]
 * [--load <id>] [--prompt "…"] [--reject] [--cancel-after 3000]`
 */

import { spawn } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { Readable, Writable } from 'node:stream'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'
import { ClientSideConnection, ndJsonStream, PROTOCOL_VERSION } from '@agentclientprotocol/sdk'

const PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: {
    prompt: { type: 'string' },
    load: { type: 'string' },
    mode: { type: 'string' },
    model: { type: 'string' },
    profile: { type: 'string' },
    cwd: { type: 'string' },
    reject: { type: 'boolean' },
    'cancel-after': { type: 'string' },
  },
  strict: true,
})

/** Which option this run answers every permission request with. */
const PERMISSION_CHOICE = values.reject === true ? 'reject-once' : 'allow-once'

/** The workspace the session opens against — an IDE passes the user's project. */
const WORKSPACE = resolve(process.cwd(), values.cwd ?? PACKAGE_ROOT)

// Two launch routes, one composition. `--profile` is what BitFun ships: the
// user's installed `dsh` boots the materialized profile under $DSH_HOME, and
// every harness package resolves from that installation. Without it, this runs
// the working copy through tsx, which is the development loop.
const launch = values.profile === undefined
  ? { command: process.execPath, args: ['--import', 'tsx', 'src/bin.ts', '--config', './cordis.yml'] }
  : { command: 'dsh', args: ['--profile', values.profile] }

const child = spawn(
  launch.command,
  launch.args,
  { cwd: PACKAGE_ROOT, stdio: ['pipe', 'pipe', 'inherit'] },
)

const stream = ndJsonStream(
  Writable.toWeb(child.stdin),
  Readable.toWeb(child.stdout),
)

/** Print one session update in a single readable line. */
function describe(update) {
  const kind = update.sessionUpdate
  if (kind === 'tool_call' || kind === 'tool_call_update') {
    const label = update.title === undefined ? '' : ` ${update.title}`
    return `${kind} ${update.toolCallId}${label} [${update.status ?? '-'}]`
  }
  if (kind === 'agent_message_chunk' || kind === 'agent_thought_chunk') {
    return `${kind} ${JSON.stringify(update.content.text ?? '')}`
  }
  if (kind === 'plan') return `plan ${update.entries.map(entry => `${entry.status}:${entry.content}`).join(' | ')}`
  if (kind === 'config_option_update') {
    return `config_option_update ${describeOptions(update.configOptions)}`
  }
  return kind
}

/** Print a config option set the way a client's picker would show it. */
function describeOptions(configOptions) {
  if (configOptions === undefined || configOptions.length === 0) return '(none)'
  return configOptions.map(option => {
    // A select's values are either flat or grouped by provider; a picker
    // renders both as one list, so this flattens the grouped form too.
    const values = option.type === 'select'
      ? option.options
        .flatMap(entry => (entry.options === undefined ? [entry] : entry.options))
        .map(value => (value.value === option.currentValue ? `[${value.value}]` : value.value))
        .join(' ')
      : String(option.currentValue)
    return `${option.id}(${option.category ?? '-'}): ${values}`
  }).join(' | ')
}

const client = new ClientSideConnection(() => ({
  sessionUpdate(params) {
    process.stdout.write(`  update: ${describe(params.update)}\n`)
    return Promise.resolve()
  },
  requestPermission(params) {
    process.stdout.write(
      `  permission: ${params.toolCall.toolCallId} (${params.toolCall.title ?? '?'}) -> ${PERMISSION_CHOICE}\n`,
    )
    return Promise.resolve({ outcome: { outcome: 'selected', optionId: PERMISSION_CHOICE } })
  },
}), stream)

let failed = false
try {
  const initialized = await client.initialize({ protocolVersion: PROTOCOL_VERSION, clientCapabilities: {} })
  process.stdout.write(`initialize: ${JSON.stringify(initialized.agentInfo)} v${initialized.protocolVersion}\n`)
  process.stdout.write(`loadSession capability: ${String(initialized.agentCapabilities?.loadSession === true)}\n`)

  // `--load` is the reopen path an IDE takes on restart: the session comes back
  // from storage with its history replayed as updates and its mode already
  // fixed, instead of a blank session under the roster default.
  const session = values.load === undefined
    ? await client.newSession({ cwd: WORKSPACE, mcpServers: [] })
    : { sessionId: values.load, ...await client.loadSession({ sessionId: values.load, cwd: WORKSPACE, mcpServers: [] }) }
  const sessionId = session.sessionId
  process.stdout.write(`${values.load === undefined ? 'newSession' : 'loadSession'}: ${sessionId}\n`)
  process.stdout.write(`options: ${describeOptions(session.configOptions)}\n`)

  if (values.mode !== undefined) {
    const switched = await client.setSessionConfigOption({
      sessionId,
      configId: 'agent-preset',
      value: values.mode,
    })
    process.stdout.write(`mode ${values.mode}: ${describeOptions(switched.configOptions)}\n`)
  }

  // `--model provider/model` is the composer's model dropdown: unlike the mode
  // it stays live for the whole session, so this can follow a prompt too.
  if (values.model !== undefined) {
    const switched = await client.setSessionConfigOption({
      sessionId,
      configId: 'model',
      value: values.model,
    })
    process.stdout.write(`model ${values.model}: ${describeOptions(switched.configOptions)}\n`)
  }

  if (values.prompt !== undefined) {
    const pending = client.prompt({ sessionId, prompt: [{ type: 'text', text: values.prompt }] })
    if (values['cancel-after'] !== undefined) {
      await new Promise(resolve => setTimeout(resolve, Number(values['cancel-after'])))
      process.stdout.write('cancel: sent\n')
      await client.cancel({ sessionId })
    }
    const result = await pending
    process.stdout.write(`prompt: ${result.stopReason}\n`)

    if (values['cancel-after'] !== undefined) {
      // The point of the cancel probe: the session must still take work.
      const next = await client.prompt({ sessionId, prompt: [{ type: 'text', text: 'say OK and nothing else' }] })
      process.stdout.write(`prompt after cancel: ${next.stopReason}\n`)
    }
  }
} catch (error) {
  failed = true
  process.stdout.write(`FAILED: ${String(error)}\n`)
}

child.stdin.end()
await new Promise(resolve => child.once('exit', resolve))
process.exit(failed ? 1 : 0)
