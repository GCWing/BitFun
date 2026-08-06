export function supportsResponsesReasoning(provider?: string): boolean {
  return provider === 'response' || provider === 'responses';
}
