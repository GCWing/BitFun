const CARD_GRADIENTS = [
  'var(--app-card-gradient-0)',
  'var(--app-card-gradient-1)',
  'var(--app-card-gradient-2)',
  'var(--app-card-gradient-3)',
  'var(--app-card-gradient-4)',
];

function getCardGradient(seed: string): string {
  const first = seed.trim().charCodeAt(0) || 0;
  return CARD_GRADIENTS[first % CARD_GRADIENTS.length];
}

export { getCardGradient };
