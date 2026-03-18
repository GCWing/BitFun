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

export async function checkConnectivity(url: string): Promise<void> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 5000);
  try {
    await fetch(url, {
      method: 'HEAD',
      mode: 'no-cors',
      signal: controller.signal,
    });
  } catch {
    throw new Error(`Connection failed: ${new URL(url).hostname} is not reachable`);
  } finally {
    clearTimeout(timeout);
  }
}
