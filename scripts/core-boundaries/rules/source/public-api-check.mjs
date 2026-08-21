// Public API allowlist check logic. Kept separate from checker.mjs so the
// checker stays a thin orchestrator (under the 1200-line module budget).

function collectRustUseReexportSymbols(usePath) {
  const blockMatch = usePath.match(/\{([\s\S]*)\}$/);
  if (blockMatch) {
    const prefix = usePath.slice(0, blockMatch.index).replace(/::$/, '');
    return blockMatch[1].split(',').flatMap((symbol) => {
      symbol = symbol.trim();
      return symbol ? collectRustUseReexportSymbols(`${prefix}::${symbol}`) : [];
    });
  }

  const aliasMatch = usePath.match(/\bas\s+([A-Za-z_][A-Za-z0-9_]*)$/);
  const symbol =
    aliasMatch?.[1] ??
    usePath
      .split('::')
      .map((part) => part.trim())
      .filter(Boolean)
      .pop();
  return symbol ? [symbol] : [];
}

export function collectTopLevelRustPublicSymbols(text) {
  const symbols = Array.from(text.matchAll(/\bexternal_subagent_id!\(\s*([A-Za-z_][A-Za-z0-9_]*)/g), (match) => match[1]);
  let braceDepth = 0;
  let pendingUsePath = null;
  for (const line of text.split(/\r?\n/)) {
    const code = line.replace(/\/\/.*$/, '');
    if (pendingUsePath) {
      pendingUsePath.push(code.trim());
      if (code.includes(';')) {
        symbols.push(
          ...collectRustUseReexportSymbols(pendingUsePath.join(' ').replace(/;\s*$/, '')),
        );
        pendingUsePath = null;
      }
      continue;
    }

    if (braceDepth === 0) {
      const useMatch = code.match(/^\s*pub\s+use\s+(.+)/);
      if (useMatch) {
        const usePath = useMatch[1].trim();
        if (usePath.includes(';')) {
          symbols.push(...collectRustUseReexportSymbols(usePath.replace(/;\s*$/, '')));
        } else {
          pendingUsePath = [usePath];
        }
        continue;
      }
      const match = line.match(
        /^\s*pub\s+(?:(?:async|unsafe)\s+)*(?:(?:const\s+fn)|fn|type|struct|enum|trait|mod|const|static)\s+([A-Za-z_][A-Za-z0-9_]*)\b/,
      );
      if (match) {
        symbols.push(match[1]);
      }
    }
    braceDepth += (code.match(/\{/g) || []).length;
    braceDepth -= (code.match(/\}/g) || []).length;
    if (braceDepth < 0) {
      braceDepth = 0;
    }
  }
  return symbols;
}

export function collectPluginRootReexports(text) {
  const symbols = [];
  const publicName = (symbol) => symbol.split(/\s+as\s+/).pop().trim();
  const blockRegex = /\bpub\s+use\s+(?:crate::|self::)?plugin::\{([\s\S]*?)\};/g;
  for (const match of text.matchAll(blockRegex)) {
    symbols.push(
      ...match[1]
        .split(',')
        .map((symbol) => symbol.trim())
        .filter(Boolean)
        .map(publicName),
    );
  }
  const singleRegex = /\bpub\s+use\s+(?:crate::|self::)?plugin::([A-Za-z_][A-Za-z0-9_]*|\*)(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?\s*;/g;
  for (const match of text.matchAll(singleRegex)) symbols.push(match[2] || match[1]);
  return symbols;
}

export const hasPluginWildcardReexport = (text) => /\bpub\s+use\s+(?:crate::|self::)?plugin::\*/.test(text);

function allowedSymbolsForRule(rule, entriesField, symbolsField) {
  if (rule[entriesField]) {
    return rule[entriesField].map((entry) => entry.symbol);
  }
  return rule[symbolsField] || [];
}

function checkPublicApiEntryMetadata(path, entries, reason, failures, publicApiContractSliceSet) {
  if (!entries) return;
  const fail = (entry, message) =>
    failures.push({ path, line: 1, message: `${reason}; public API entry ${entry.symbol || '<missing>'} ${message}` });
  const requiredFields = ['symbol', 'owner', 'consumer', 'verification', 'p0', 'contractSlice', 'rationale', 'exit'];
  for (const entry of entries) {
    for (const field of requiredFields) {
      if (typeof entry[field] !== 'string' || entry[field].trim().length === 0) {
        fail(entry, `is missing ${field}`);
      }
    }
    if (typeof entry.wireImpact !== 'boolean') {
      fail(entry, 'must declare wireImpact');
    }
    if (!publicApiContractSliceSet.has(entry.contractSlice)) {
      fail(entry, 'has unknown contractSlice');
    }
  }
}

function compareSymbolAllowlist(path, actualSymbols, allowedSymbols, reason, failures) {
  const allowed = new Set(allowedSymbols);
  const actual = new Set(actualSymbols);
  for (const symbol of actual) {
    if (!allowed.has(symbol)) {
      failures.push({
        path,
        line: 1,
        message: `${reason}; unexpected public symbol: ${symbol}`,
      });
    }
  }
  for (const symbol of allowed) {
    if (!actual.has(symbol)) {
      failures.push({
        path,
        line: 1,
        message: `${reason}; missing public symbol: ${symbol}`,
      });
    }
  }
}

export function runPublicApiChecks(rules, { failures, repoPathToFsPath, readText, publicApiContractSliceSet }) {
  for (const rule of rules) {
    const path = repoPathToFsPath(rule.path);
    const text = readText(path);
    checkPublicApiEntryMetadata(path, rule.allowedSymbolEntries, rule.reason, failures, publicApiContractSliceSet);
    checkPublicApiEntryMetadata(path, rule.allowedPluginReexportEntries, rule.reason, failures, publicApiContractSliceSet);
    if (rule.allowedSymbols || rule.allowedSymbolEntries) {
      compareSymbolAllowlist(
        path,
        collectTopLevelRustPublicSymbols(text),
        allowedSymbolsForRule(rule, 'allowedSymbolEntries', 'allowedSymbols'),
        rule.reason,
        failures,
      );
    }
    if (rule.allowedPluginReexports || rule.allowedPluginReexportEntries) {
      compareSymbolAllowlist(
        path,
        collectPluginRootReexports(text),
        allowedSymbolsForRule(rule, 'allowedPluginReexportEntries', 'allowedPluginReexports'),
        rule.reason,
        failures,
      );
      if (hasPluginWildcardReexport(text)) {
        failures.push({
          path,
          line: 1,
          message: `${rule.reason}; wildcard plugin re-export is forbidden`,
        });
      }
    }
  }
}
