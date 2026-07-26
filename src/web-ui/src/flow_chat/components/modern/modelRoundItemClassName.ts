/**
 * Build ModelRoundItem root class names.
 *
 * Important: rounds that begin streaming must not later pick up an enter
 * animation when they flip to complete — that replays opacity 0→1 and looks
 * like the whole conversation refreshed.
 */
export function getModelRoundItemClassName(params: {
  isVisuallyStreaming: boolean;
  shouldPlayEnterAnimation: boolean;
}): string {
  const { isVisuallyStreaming, shouldPlayEnterAnimation } = params;
  return [
    'model-round-item',
    isVisuallyStreaming ? 'model-round-item--streaming' : 'model-round-item--complete',
    shouldPlayEnterAnimation ? 'model-round-item--enter' : '',
  ].filter(Boolean).join(' ');
}
