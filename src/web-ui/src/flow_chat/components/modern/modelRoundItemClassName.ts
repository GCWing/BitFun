/**
 * Build ModelRoundItem root class names.
 *
 * Important: there is deliberately no mount / enter animation modifier. The
 * list is virtualized, so any mount-triggered animation replays every time an
 * item scrolls back into view, which reads as the whole conversation flashing.
 */
export function getModelRoundItemClassName(params: {
  isVisuallyStreaming: boolean;
}): string {
  const { isVisuallyStreaming } = params;
  return [
    'model-round-item',
    isVisuallyStreaming ? 'model-round-item--streaming' : 'model-round-item--complete',
  ].join(' ');
}
