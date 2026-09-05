// Export reviewable coverage worksheets; this is not a test or a second registry.
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--output') {
  throw new Error('Usage: node scripts/diagnostics/export-communications-matrix.mjs --output <new-directory>');
}
const output = resolve(args[1]);
const registry = JSON.parse(await readFile(resolve(root,
  'src/crates/contracts/product-domains/src/generated/remote-surface-registry.json'), 'utf8'));
if (registry.schemaVersion !== 1 || !Array.isArray(registry.operations)) {
  throw new Error('Unsupported Product Operation Registry shape');
}
const source = await readFile(resolve(root,
  'src/crates/services/services-integrations/src/remote_connect.rs'), 'utf8');
const commandBody = source.match(/pub enum RemoteCommand \{\n([\s\S]*?)\n\}/)?.[1];
if (!commandBody) throw new Error('RemoteCommand enum was not found');
const commands = [...commandBody.matchAll(/^    ([A-Z]\w*)\s*(?:,|\{)/gm)]
  .map((match) => match[1].replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase());
if (commands.length === 0 || new Set(commands).size !== commands.length) {
  throw new Error('RemoteCommand inventory is empty or contains duplicates');
}
const guide = await readFile(resolve(root, 'docs/development/communications-e2e.zh-CN.md'), 'utf8');
const cases = [...guide.matchAll(/^\| ((?:ENV|RLY|SSH|MOB|BOT|PEER|DSP|COMBO|EXT)-\d+) \| ([^\n]+) \| ([^\n]+) \|$/gm)]
  .map((match) => [match[1], match[2], match[3]]);
if (cases.length === 0 || new Set(cases.map(([id]) => id)).size !== cases.length) {
  throw new Error('Manual case inventory is empty or contains duplicate ids');
}
const csv = (rows) => rows.map((row) => row.map((cell) =>
  `"${String(cell ?? '').replaceAll('"', '""')}"`).join(',')).join('\n') + '\n';
// A fresh directory prevents a rerun from erasing manually recorded evidence.
await mkdir(output, { recursive: false });
await writeFile(resolve(output, 'product-operations.csv'), csv([
  ['operation', 'remote_workspace_policy', 'peer_policy', 'cli_peer_policy', 'cli_reason',
    'ssh_result', 'desktop_peer_result', 'cli_peer_result', 'composition_result', 'evidence'],
  ...registry.operations.map((op) => [op.id, op.remoteWorkspace, op.peer.kind, op.cliPeer.kind,
    op.cliPeer.reason, 'NOT_RUN', 'NOT_RUN', 'NOT_RUN', 'NOT_RUN', '']),
]));
await writeFile(resolve(output, 'remote-commands.csv'), csv([
  ['command', 'room_mobile_result', 'account_mobile_result', 'feishu_result',
    'telegram_result', 'weixin_result', 'peer_result', 'evidence'],
  ...commands.map((command) => [command, ...Array(6).fill('NOT_RUN'), '']),
]));
await writeFile(resolve(output, 'manual-cases.csv'), csv([
  ['case_id', 'steps', 'expected', 'environment', 'result', 'evidence', 'issue'],
  ...cases.map(([id, steps, expected]) => [id, steps, expected, '', 'NOT_RUN', '', '']),
]));
const inventory = {
  registryDigest: registry.digest,
  productOperations: registry.operations.length,
  remoteCommands: commands.length,
  manualCases: cases.length,
  note: 'Inventory only. NOT_RUN is not coverage. Preserve environment and evidence per execution.',
};
await writeFile(resolve(output, 'inventory.json'), JSON.stringify(inventory, null, 2) + '\n');
console.log(JSON.stringify(inventory, null, 2));
