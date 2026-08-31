export const DEFAULT_CARD_GRADIENT =
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-color-accent-hover) 28%, transparent) 0%, color-mix(in srgb, var(--bf-color-accent-secondary) 18%, transparent) 100%)';

const CARD_GRADIENTS = [
  DEFAULT_CARD_GRADIENT,
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-color-status-success-content) 24%, transparent) 0%, color-mix(in srgb, var(--bf-color-accent-hover) 18%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-color-status-warning-content) 22%, transparent) 0%, color-mix(in srgb, var(--bf-color-status-danger-content) 16%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-color-accent-secondary) 28%, transparent) 0%, color-mix(in srgb, var(--bf-color-status-danger-content) 18%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-color-status-info-content) 22%, transparent) 0%, color-mix(in srgb, var(--bf-color-accent-hover) 18%, transparent) 100%)',
];

function getCardGradient(seed: string): string {
  const first = seed.trim().charCodeAt(0) || 0;
  return CARD_GRADIENTS[first % CARD_GRADIENTS.length];
}

export { getCardGradient };
