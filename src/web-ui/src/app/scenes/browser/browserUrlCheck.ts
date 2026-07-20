export function validateUrl(url: string): void {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      throw new Error(`Unsupported protocol: ${parsed.protocol}`);
    }
    if (!parsed.hostname) {
      throw new Error('Missing hostname');
    }
  } catch (e) {
    throw new Error(`Invalid URL: ${url}${e instanceof Error ? ` (${e.message})` : ''}`);
  }
}
