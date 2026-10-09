export type SpeechEndpoint = { baseUrl: string; token: string };

export function speechUrl(value: string): string {
  const url = new URL(value.trim());
  if (
    url.protocol !== 'https:' ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    url.pathname !== '/'
  ) {
    throw new Error(
      'Use an HTTPS speech service URL without a path or credentials',
    );
  }
  return url.origin;
}

export function speechToken(token: string): string {
  if (!token.trim() || /[\r\n]/.test(token))
    throw new Error('Enter the speech service token');
  return token.trim();
}
