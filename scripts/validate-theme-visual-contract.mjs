import fs from 'node:fs';

const CONTRACT_URL = new URL('./theme-visual-governance-contract.json', import.meta.url);
const REQUIRED_SURFACE_KEYS = [
  'app-shell',
  'flow-chat',
  'tool-cards-review',
  'code-editor-diff',
  'terminal',
  'markdown-mermaid',
  'generated-widget',
  'theme-settings',
  'mobile-web-shell',
  'installer-shell',
];
const ALLOWED_PLATFORMS = new Set(['desktop-webview', 'web', 'mobile-web', 'generated-widget', 'installer']);
const ALLOWED_FORM_FACTORS = new Set(['desktop', 'narrow', 'mobile', 'iframe']);
const ALLOWED_THEMES = new Set(['dark', 'light', 'system']);
const ALLOWED_EVIDENCE_TYPES = new Set([
  'boundary-render-review',
  'contrast-review',
  'focused-visual-review',
  'mobile-build-review',
  'theme-color-audit',
]);

function readContract() {
  try {
    return JSON.parse(fs.readFileSync(CONTRACT_URL, 'utf8'));
  } catch (error) {
    throw new Error(`Failed to parse scripts/theme-visual-governance-contract.json: ${error.message}`);
  }
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim() !== '';
}

function requireString(value, path, failures) {
  if (!isNonEmptyString(value)) {
    failures.push(`${path} must be a non-empty string`);
  }
}

function requireStringArray(value, path, failures, { allowedValues, minLength = 1 } = {}) {
  if (!Array.isArray(value)) {
    failures.push(`${path} must be an array`);
    return;
  }
  if (value.length < minLength) {
    failures.push(`${path} must contain at least ${minLength} item(s)`);
    return;
  }
  const seen = new Set();
  value.forEach((entry, index) => {
    if (!isNonEmptyString(entry)) {
      failures.push(`${path}[${index}] must be a non-empty string`);
      return;
    }
    if (seen.has(entry)) {
      failures.push(`${path}[${index}] duplicates ${entry}`);
    }
    seen.add(entry);
    if (allowedValues && !allowedValues.has(entry)) {
      failures.push(`${path}[${index}] has unsupported value ${entry}`);
    }
  });
}

function validateEvidence(surface, failures) {
  const path = `surfaces.${surface.key}.evidence`;
  if (!Array.isArray(surface.evidence) || surface.evidence.length === 0) {
    failures.push(`${path} must contain at least one evidence requirement`);
    return;
  }

  let hasActionableEvidence = false;
  surface.evidence.forEach((entry, index) => {
    const entryPath = `${path}[${index}]`;
    if (!entry || typeof entry !== 'object' || Array.isArray(entry)) {
      failures.push(`${entryPath} must be an object`);
      return;
    }
    requireString(entry.type, `${entryPath}.type`, failures);
    if (isNonEmptyString(entry.type) && !ALLOWED_EVIDENCE_TYPES.has(entry.type)) {
      failures.push(`${entryPath}.type has unsupported value ${entry.type}`);
    }
    requireString(entry.requirement, `${entryPath}.requirement`, failures);
    if (isNonEmptyString(entry.command)) {
      hasActionableEvidence = true;
    }
    if (entry.type !== 'focused-visual-review' && entry.type !== 'contrast-review') {
      hasActionableEvidence = true;
    }
  });

  if (!hasActionableEvidence) {
    failures.push(`${path} must include at least one command-backed or boundary-specific evidence requirement`);
  }
}

function validateSurface(surface, index, failures) {
  const path = `surfaces[${index}]`;
  if (!surface || typeof surface !== 'object' || Array.isArray(surface)) {
    failures.push(`${path} must be an object`);
    return;
  }

  requireString(surface.key, `${path}.key`, failures);
  if (isNonEmptyString(surface.key) && !/^[a-z0-9-]+$/.test(surface.key)) {
    failures.push(`${path}.key must be kebab-case`);
  }
  requireString(surface.owner, `${path}.owner`, failures);
  if (isNonEmptyString(surface.owner) && !surface.owner.includes('src/')) {
    failures.push(`${path}.owner must point to a source path`);
  }
  requireStringArray(surface.platforms, `${path}.platforms`, failures, { allowedValues: ALLOWED_PLATFORMS });
  requireStringArray(surface.formFactors, `${path}.formFactors`, failures, { allowedValues: ALLOWED_FORM_FACTORS });
  requireStringArray(surface.themes, `${path}.themes`, failures, { allowedValues: ALLOWED_THEMES, minLength: 2 });
  if (Array.isArray(surface.themes)) {
    for (const requiredTheme of ['dark', 'light']) {
      if (!surface.themes.includes(requiredTheme)) {
        failures.push(`${path}.themes must include ${requiredTheme}`);
      }
    }
  }
  requireStringArray(surface.states, `${path}.states`, failures, { minLength: 3 });
  requireStringArray(surface.tokenFamilies, `${path}.tokenFamilies`, failures, { minLength: 2 });
  requireStringArray(surface.risks, `${path}.risks`, failures, { minLength: 2 });
  validateEvidence(surface, failures);
}

function validateContract(contract) {
  const failures = [];
  if (!contract || typeof contract !== 'object' || Array.isArray(contract)) {
    return ['theme visual governance contract must be an object'];
  }
  if (contract.version !== 1) {
    failures.push('version must be 1');
  }
  requireString(contract.description, 'description', failures);
  if (!Array.isArray(contract.surfaces)) {
    failures.push('surfaces must be an array');
    return failures;
  }

  const surfaceKeys = new Set();
  contract.surfaces.forEach((surface, index) => {
    validateSurface(surface, index, failures);
    if (isNonEmptyString(surface?.key)) {
      if (surfaceKeys.has(surface.key)) {
        failures.push(`surfaces[${index}].key duplicates ${surface.key}`);
      }
      surfaceKeys.add(surface.key);
    }
  });

  for (const requiredKey of REQUIRED_SURFACE_KEYS) {
    if (!surfaceKeys.has(requiredKey)) {
      failures.push(`surfaces is missing required surface ${requiredKey}`);
    }
  }

  return failures;
}

const contract = readContract();
const failures = validateContract(contract);

if (failures.length > 0) {
  console.error('Theme visual governance contract failed:');
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exitCode = 1;
} else {
  console.log(
    `Theme visual governance contract: ${contract.surfaces.length} surfaces, ` +
    `${REQUIRED_SURFACE_KEYS.length} required surfaces covered.`
  );
}
