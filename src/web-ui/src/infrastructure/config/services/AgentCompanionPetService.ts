import { invoke } from '@tauri-apps/api/core';
import { readFile } from '@tauri-apps/plugin-fs';
import type { AgentCompanionPetSelection } from './AIExperienceConfigService';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('AgentCompanionPetService');
const BUILTIN_PET_BASE = '/agent-companion-pets';

/** Cache: absolute file path → blob URL (prevents re-reading the same file). */
const blobUrlCache = new Map<string, string>();

async function readFileAsBlobUrl(absolutePath: string, mimeType: string): Promise<string> {
  const cached = blobUrlCache.get(absolutePath);
  if (cached) return cached;

  const bytes = await readFile(absolutePath);
  const blob = new Blob([bytes], { type: mimeType });
  const url = URL.createObjectURL(blob);
  blobUrlCache.set(absolutePath, url);
  return url;
}

/**
 * Revoke cached blob URLs for files under a removed user pet package directory.
 */
export function releaseAgentCompanionPetPreviewBlobs(
  packagePath: string,
  spritesheetAbsolutePath?: string,
): void {
  const dir = packagePath.replace(/[/\\]+$/, '');
  const normPrefix = `${dir.replace(/\\/g, '/')}/`;
  const toRevoke = new Set<string>();
  for (const key of blobUrlCache.keys()) {
    const k = key.replace(/\\/g, '/');
    const d = dir.replace(/\\/g, '/');
    if (k === d || k.startsWith(normPrefix)) {
      toRevoke.add(key);
    }
  }
  if (spritesheetAbsolutePath) {
    toRevoke.add(spritesheetAbsolutePath);
  }
  for (const key of toRevoke) {
    const url = blobUrlCache.get(key);
    if (url) URL.revokeObjectURL(url);
    blobUrlCache.delete(key);
  }
}

const BUILTIN_PETS: AgentCompanionPetSelection[] = [
  {
    id: 'boxcat',
    displayName: 'Boxcat',
    description: 'A tiny cat tucked inside a cardboard box for cozy coding sessions.',
    source: 'preset',
    packagePath: `${BUILTIN_PET_BASE}/boxcat`,
    spritesheetPath: `${BUILTIN_PET_BASE}/boxcat/spritesheet.webp`,
    spritesheetMimeType: 'image/webp',
  },
  {
    id: 'cream-cat',
    displayName: 'Cream Cat',
    description: 'A tiny cream pixel-art cat with a heart bubble for happy moments.',
    source: 'preset',
    packagePath: `${BUILTIN_PET_BASE}/cream-cat`,
    spritesheetPath: `${BUILTIN_PET_BASE}/cream-cat/spritesheet.webp`,
    spritesheetMimeType: 'image/webp',
  },
  {
    id: 'elaina',
    displayName: 'Elaina',
    description: 'A cute pixel-art Codex pet inspired by Elaina, the tiny traveling witch with a bright hat and gentle broom-side charm.',
    source: 'preset',
    packagePath: `${BUILTIN_PET_BASE}/elaina-2`,
    spritesheetPath: `${BUILTIN_PET_BASE}/elaina-2/spritesheet.webp`,
    spritesheetMimeType: 'image/webp',
  },
  {
    id: 'gugugaga',
    displayName: '咕咕嘎嘎',
    description: 'A cheerful chibi girl in a black penguin suit with a simple silver collar pendant.',
    source: 'preset',
    packagePath: `${BUILTIN_PET_BASE}/gugugaga`,
    spritesheetPath: `${BUILTIN_PET_BASE}/gugugaga/spritesheet.webp`,
    spritesheetMimeType: 'image/webp',
  },
  {
    id: 'ikun',
    displayName: 'IKUN',
    description: 'A hoodie chick with hot path stage energy.',
    source: 'preset',
    packagePath: `${BUILTIN_PET_BASE}/ikun`,
    spritesheetPath: `${BUILTIN_PET_BASE}/ikun/spritesheet.webp`,
    spritesheetMimeType: 'image/webp',
  },
  {
    id: 'pixel-panda',
    displayName: 'Pixel Panda',
    description: 'A gentle panda holding a tiny pixel brush.',
    source: 'preset',
    packagePath: `${BUILTIN_PET_BASE}/pixel-panda`,
    spritesheetPath: `${BUILTIN_PET_BASE}/pixel-panda/spritesheet.webp`,
    spritesheetMimeType: 'image/webp',
  },
  {
    id: 'usagi',
    displayName: 'Usagi',
    description: 'A tiny cream rabbit companion based on the provided Usagi reference.',
    source: 'preset',
    packagePath: `${BUILTIN_PET_BASE}/usagi`,
    spritesheetPath: `${BUILTIN_PET_BASE}/usagi/spritesheet.webp`,
    spritesheetMimeType: 'image/webp',
  },
];

export interface AgentCompanionPetPackage extends AgentCompanionPetSelection {
  previewSrc: string;
}

interface ListAgentCompanionPetsResponse {
  pets: AgentCompanionPetSelection[];
}

async function withPreviewSrc(pet: AgentCompanionPetSelection): Promise<AgentCompanionPetPackage> {
  const previewSrc = pet.source === 'preset'
    ? pet.spritesheetPath
    : isTauriRuntime()
      ? await readFileAsBlobUrl(pet.spritesheetPath, pet.spritesheetMimeType)
      : '';
  return { ...pet, previewSrc };
}

export async function listAgentCompanionPets(): Promise<AgentCompanionPetPackage[]> {
  const builtinPets = await Promise.all(BUILTIN_PETS.map(withPreviewSrc));
  if (!isTauriRuntime()) return builtinPets;
  try {
    const response = await invoke<ListAgentCompanionPetsResponse>('list_agent_companion_pets');
    const userPets = await Promise.all(response.pets.map(withPreviewSrc));
    return [...builtinPets, ...userPets];
  } catch (error) {
    log.error('Failed to list Agent companion pets', error);
    return builtinPets;
  }
}

export async function importAgentCompanionPetPackage(path: string): Promise<AgentCompanionPetPackage> {
  const pet = await invoke<AgentCompanionPetSelection>('import_agent_companion_pet_package', {
    request: { path },
  });
  return withPreviewSrc(pet);
}

export async function deleteAgentCompanionPetPackage(packagePath: string): Promise<void> {
  await invoke('delete_agent_companion_pet_package', {
    request: { packagePath },
  });
}

/**
 * Resolve the image source for a pet's spritesheet.
 * For preset pets, returns the web-public relative path directly.
 * For user-imported pets, reads the file from disk and returns a blob URL (cached).
 */
export async function resolveAgentCompanionPetSrc(
  pet: AgentCompanionPetSelection | null | undefined,
): Promise<string> {
  if (!pet) return '';
  if (pet.source === 'preset') return pet.spritesheetPath;
  if (!isTauriRuntime()) return '';
  return readFileAsBlobUrl(pet.spritesheetPath, pet.spritesheetMimeType);
}
