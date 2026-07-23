export const DEFAULT_CARD_GRADIENT =
  'linear-gradient(135deg, color-mix(in srgb, var(--color-accent-600) 28%, transparent) 0%, color-mix(in srgb, var(--color-purple-500) 18%, transparent) 100%)';

const CARD_GRADIENTS = [
  DEFAULT_CARD_GRADIENT,
  'linear-gradient(135deg, color-mix(in srgb, var(--color-success) 24%, transparent) 0%, color-mix(in srgb, var(--color-accent-600) 18%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--color-warning) 22%, transparent) 0%, color-mix(in srgb, var(--color-error) 16%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--color-purple-500) 28%, transparent) 0%, color-mix(in srgb, var(--color-error) 18%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--color-cyan-500) 22%, transparent) 0%, color-mix(in srgb, var(--color-accent-600) 18%, transparent) 100%)',
];

function getCardGradient(seed: string): string {
  const first = seed.trim().charCodeAt(0) || 0;
  return CARD_GRADIENTS[first % CARD_GRADIENTS.length];
}

export { getCardGradient };
