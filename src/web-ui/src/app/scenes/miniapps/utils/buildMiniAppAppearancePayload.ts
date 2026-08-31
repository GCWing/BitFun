import {
  cssVariables as systemCssVariables,
  tokens as systemTokens,
  type TokenName as SystemTokenName,
} from '@bitfun/design-tokens';
import type {
  AppearanceMode,
  AppearanceThemeTokenName,
  ResolvedAppearance,
} from '@/infrastructure/appearance';
import miniAppAppearanceContract from '../../../../../../shared/miniapp-appearance/contract.json';

export interface MiniAppAppearancePayload {
  mode: AppearanceMode;
  id: string;
  vars: Record<string, string>;
}

interface MiniAppAppearanceVariableContract {
  readonly name: `--bitfun-${string}`;
  readonly kind: 'theme' | 'system';
  readonly source: `--bf-${string}`;
}

const APPEARANCE_VARIABLES = miniAppAppearanceContract.variables as readonly MiniAppAppearanceVariableContract[];
const SYSTEM_TOKEN_NAMES_BY_CSS_VARIABLE = new Map<string, SystemTokenName>(
  (Object.entries(systemCssVariables) as [SystemTokenName, `--bf-${string}`][])
    .map(([name, cssVariable]) => [cssVariable, name]),
);

function getThemeTokens(appearance: ResolvedAppearance): Record<string, string> | null {
  const settings = appearance.renderers['theme-tokens'];
  const tokens = settings?.tokens;
  if (!tokens || typeof tokens !== 'object' || Array.isArray(tokens)) return null;
  return tokens as Record<string, string>;
}

export function buildMiniAppAppearancePayload(
  appearance: ResolvedAppearance | null,
): MiniAppAppearancePayload | null {
  if (!appearance) return null;
  const tokens = getThemeTokens(appearance);
  if (!tokens) return null;
  const vars: Record<string, string> = {};
  APPEARANCE_VARIABLES.forEach((variable) => {
    if (variable.kind === 'theme') {
      const value = tokens[variable.source as AppearanceThemeTokenName];
      if (typeof value === 'string') vars[variable.name] = value;
      return;
    }

    const tokenName = SYSTEM_TOKEN_NAMES_BY_CSS_VARIABLE.get(variable.source);
    if (!tokenName) {
      throw new Error(`Unknown MiniApp system token source: ${variable.source}`);
    }
    vars[variable.name] = String(systemTokens[tokenName]);
  });
  return { mode: appearance.mode, id: appearance.id, vars };
}
