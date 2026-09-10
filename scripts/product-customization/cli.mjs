#!/usr/bin/env node
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { ProductDefinitionError, resolveProductDefinition } from './resolver.mjs';

export function extractProductConfigArg(args) {
  const forwardArgs = [];
  let productConfig;
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    const inline = argument.startsWith('--product-config=')
      ? argument.slice('--product-config='.length)
      : undefined;
    if (argument === '--product-config' || inline !== undefined) {
      const value = inline ?? args[++index];
      if (!value || value === '--') {
        throw new ProductDefinitionError(
          'missing_product_config_value',
          '--product-config requires a file path.',
          'Pass --product-config products/example/product.jsonc.',
        );
      }
      if (productConfig !== undefined) {
        throw new ProductDefinitionError(
          'duplicate_product_config',
          '--product-config was provided more than once.',
          'Keep one explicit product-definition path.',
        );
      }
      productConfig = value;
    } else {
      forwardArgs.push(argument);
    }
  }
  return { productConfig, forwardArgs };
}

function diagnosticArgs(args) {
  const { productConfig, forwardArgs } = extractProductConfigArg(args);
  let member = 'desktop';
  for (let index = 0; index < forwardArgs.length; index += 1) {
    const argument = forwardArgs[index];
    if (argument === '--' && index === 0) continue;
    if (argument === '--member') member = forwardArgs[++index];
    else if (argument.startsWith('--member=')) member = argument.slice('--member='.length);
    else throw new ProductDefinitionError('unknown_diagnostic_option', `Unsupported option: ${argument}`, 'Use only --product-config and --member.');
  }
  return { productConfig, member };
}

export function explainProduct(resolution) {
  return {
    definition: resolution.sourcePath,
    member: resolution.assembly.member,
    binaryName: resolution.assembly.binaryName,
    bundleId: resolution.assembly.bundleId,
    localizedNames: resolution.productNames,
    localeDigest: resolution.assembly.localeDigest,
    assemblyDigest: resolution.assembly.assemblyDigest,
    implementedScope: 'identity-and-localized-name-c0b',
  };
}

async function main() {
  const rootDir = resolve(import.meta.dirname, '..', '..');
  const [command = 'check', ...args] = process.argv.slice(2);
  const { productConfig, member } = diagnosticArgs(args);
  const resolution = resolveProductDefinition({ rootDir, productConfig, member });
  if (command === 'check') {
    console.log(JSON.stringify({
      ok: true,
      member,
      assemblyDigest: resolution.assembly.assemblyDigest,
    }));
    return;
  }
  if (command === 'explain') {
    console.log(JSON.stringify(explainProduct(resolution), null, 2));
    return;
  }
  throw new ProductDefinitionError('unknown_product_command', `Unknown command: ${command}`, 'Use check or explain.');
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    if (error instanceof ProductDefinitionError) {
      console.error(JSON.stringify({ ok: false, code: error.code, message: error.message, action: error.action }));
    } else {
      console.error(error?.stack || error);
    }
    process.exitCode = 1;
  });
}
