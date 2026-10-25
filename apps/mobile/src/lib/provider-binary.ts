import type { ProviderKind } from '@waku/client';

/**
 * The binary a start request carries. Nebius is a native driver with no
 * executable: its probe reports installed with a null path, and the daemon
 * expects an empty binary (desktop sends an empty PathBuf).
 */
export function providerStartBinary(provider: ProviderKind, probe: { installed: boolean; path: string | null }): string {
  if (provider === 'nebius' && probe.installed) return probe.path ?? '';
  if (!probe.installed || !probe.path) throw new Error(`${provider} is not installed on the daemon host`);
  return probe.path;
}
