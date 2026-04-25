/**
 * Panel configuration - defines panel state model and threshold constants.
 *
 * Design references:
 * - VS Code: three modes (hidden/narrow/wide) + shortcuts + width memory
 * - JetBrains: multi-mode (floating/fixed/collapsed/icon)
 * - Figma: smart snapping + smooth animations
 *
 * Panel display modes:
 * - collapsed: fully hidden
 * - compact: compact mode (icons only, good for small screens)
 * - comfortable: comfortable mode (standard content layout)
 * - expanded: expanded mode (wide content, more information)
 */

import { storage } from '@/shared';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('PanelConfig');

// ==================== Panel display modes ====================
export type PanelDisplayMode = 'collapsed' | 'compact' | 'comfortable' | 'expanded';

// ==================== Left panel config ====================
export const LEFT_PANEL_CONFIG = {
  // Width thresholds (px)
  COLLAPSED_WIDTH: 0,           // Fully collapsed
  COMPACT_WIDTH: 200,           // Compact mode fixed width - also the minimum drag width
  COMPACT_THRESHOLD: 140,       // Below this value enters compact mode
  COMFORTABLE_MIN: 160,         // Comfortable mode minimum width
  COMFORTABLE_DEFAULT: 280,     // Comfortable mode default width
  EXPANDED_THRESHOLD: 360,      // Above this value enters expanded mode
  MAX_WIDTH: 500,               // Max width

  // Snap points (snap positions during drag)
  SNAP_POINTS: [200, 280, 360, 500],
  SNAP_RANGE: 15,               // Snap range (px)

  // Animation
  TRANSITION_DURATION: 200,     // Mode transition duration (ms)
} as const;

// ==================== Right panel config ====================
export const RIGHT_PANEL_CONFIG = {
  // Width thresholds (px)
  COLLAPSED_WIDTH: 0,           // Fully collapsed
  COMPACT_WIDTH: 300,           // Compact mode minimum width - also the minimum drag width
  COMPACT_THRESHOLD: 350,       // Below this value enters compact mode
  COMFORTABLE_MIN: 400,         // Comfortable mode minimum width
  COMFORTABLE_DEFAULT: 540,     // Comfortable mode default width (>520px for config-center tabs)
  EXPANDED_THRESHOLD: 700,      // Above this value enters expanded mode
  MAX_WIDTH: 1200,              // Max width

  // Snap points
  SNAP_POINTS: [300, 400, 540, 700, 900],
  SNAP_RANGE: 20,               // Snap range (px)

  // Animation
  TRANSITION_DURATION: 200,     // Mode transition duration (ms)
} as const;

// ==================== Common config ====================
export const PANEL_COMMON_CONFIG = {
  RESIZER_WIDTH: 4,             // Resizer width
  RESIZE_STEP: 10,              // Keyboard resize step
  RESIZE_STEP_SHIFT: 50,        // Shift key accelerated step
  MIN_CENTER_WIDTH: 400,        // Minimum center panel width
  TOUCH_THRESHOLD: 150,         // Touch device delay threshold (ms)
  DOUBLE_CLICK_DELAY: 300,      // Double-click detection delay (ms)
} as const;

// ==================== Shortcut config ====================
export const PANEL_SHORTCUTS = {
  TOGGLE_LEFT: { key: '\\', ctrlOrMeta: true },       // Ctrl/Cmd + \ toggle left
  TOGGLE_RIGHT: { key: ']', ctrlOrMeta: true },       // Ctrl/Cmd + ] toggle right
  TOGGLE_BOTH: { key: '0', ctrlOrMeta: true },        // Ctrl/Cmd + 0 toggle both
  EXPAND_LEFT: { key: '[', ctrlOrMeta: true, shift: true },   // Ctrl/Cmd + Shift + [ expand left
  EXPAND_RIGHT: { key: ']', ctrlOrMeta: true, shift: true },  // Ctrl/Cmd + Shift + ] expand right
} as const;

// ==================== Utility functions ====================

/**
 * Get panel display mode by width.
 */
export function getPanelDisplayMode(
  width: number,
  config: typeof LEFT_PANEL_CONFIG | typeof RIGHT_PANEL_CONFIG
): PanelDisplayMode {
  if (width <= 0) return 'collapsed';
  if (width < config.COMPACT_THRESHOLD) return 'compact';
  if (width >= config.EXPANDED_THRESHOLD) return 'expanded';
  return 'comfortable';
}

/**
 * Get recommended width for a mode.
 */
export function getModeWidth(
  mode: PanelDisplayMode,
  config: typeof LEFT_PANEL_CONFIG | typeof RIGHT_PANEL_CONFIG
): number {
  switch (mode) {
    case 'collapsed':
      return config.COLLAPSED_WIDTH;
    case 'compact':
      return config.COMPACT_WIDTH;
    case 'comfortable':
      return config.COMFORTABLE_DEFAULT;
    case 'expanded':
      return config.EXPANDED_THRESHOLD;
    default:
      return config.COMFORTABLE_DEFAULT;
  }
}

/**
 * Compute snapped width.
 * @param width Current width
 * @param config Panel configuration
 * @param isDragging Whether dragging is in progress (only snap on release)
 * @returns Snapped width
 */
export function getSnappedWidth(
  width: number,
  config: typeof LEFT_PANEL_CONFIG | typeof RIGHT_PANEL_CONFIG,
  isDragging: boolean = false
): number {
  // Do not force snap while dragging; return original width.
  if (isDragging) return width;

  // Check if within snap range
  for (const snapPoint of config.SNAP_POINTS) {
    if (Math.abs(width - snapPoint) <= config.SNAP_RANGE) {
      return snapPoint;
    }
  }
  
  return width;
}

/**
 * Get next mode.
 * Used for double-click toggle: compact <-> comfortable <-> expanded
 */
export function getNextMode(currentMode: PanelDisplayMode): PanelDisplayMode {
  switch (currentMode) {
    case 'collapsed':
      return 'comfortable';
    case 'compact':
      return 'comfortable';
    case 'comfortable':
      return 'expanded';
    case 'expanded':
      return 'comfortable';
    default:
      return 'comfortable';
  }
}

/**
 * Validate and clamp width within valid range.
 */
export function clampWidth(
  width: number,
  config: typeof LEFT_PANEL_CONFIG | typeof RIGHT_PANEL_CONFIG,
  containerWidth?: number
): number {
  let maxWidth: number = config.MAX_WIDTH;
  
  // If container width is provided, compute dynamic max width
  if (containerWidth) {
    const dynamicMax = containerWidth - PANEL_COMMON_CONFIG.MIN_CENTER_WIDTH - PANEL_COMMON_CONFIG.RESIZER_WIDTH;
    maxWidth = Math.min(config.MAX_WIDTH, dynamicMax);
  }
  
  // Width cannot be less than compact width (unless collapsed)
  const minWidth: number = config.COMPACT_WIDTH;
  
  return Math.max(minWidth, Math.min(maxWidth, width));
}

// ==================== Local storage keys ====================
export const STORAGE_KEYS = {
  LEFT_PANEL_WIDTH: 'bitfun:leftPanelWidth',
  RIGHT_PANEL_WIDTH: 'bitfun:rightPanelWidth',
  LEFT_PANEL_COLLAPSED: 'bitfun:leftPanelCollapsed',
  RIGHT_PANEL_COLLAPSED: 'bitfun:rightPanelCollapsed',
  LEFT_PANEL_LAST_WIDTH: 'bitfun:leftPanelLastWidth',   // Remembered width before collapse
  RIGHT_PANEL_LAST_WIDTH: 'bitfun:rightPanelLastWidth', // Remembered width before collapse
} as const;

/**
 * Save panel width to local storage.
 */
export function savePanelWidth(key: string, width: number): void {
  try {
    storage.setItem(key, String(width));
  } catch (e) {
    log.warn('Failed to save panel width', { key, width, error: e });
  }
}

/**
 * Load panel width from local storage.
 */
export function loadPanelWidth(key: string, defaultValue: number): number {
  try {
    const stored = storage.getItem(key);
    if (stored) {
      const parsed = parseInt(stored, 10);
      if (!isNaN(parsed) && parsed > 0) {
        return parsed;
      }
    }
  } catch (e) {
    log.warn('Failed to load panel width', { key, defaultValue, error: e });
  }
  return defaultValue;
}

