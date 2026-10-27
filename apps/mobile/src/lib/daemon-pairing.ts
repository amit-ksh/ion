import {
  isPrivateDaemonAddress,
  normalizeDaemonAddress,
  type DaemonProfileInput,
} from './daemon-profile';

/** Pairing codes only populate the editor. Saving remains an explicit user action. */
export function parseDaemonPairingCode(
  data: string,
): Required<DaemonProfileInput> {
  const invalid = () =>
    new Error('This is not a valid Ion desktop pairing code.');
  if (!data || data.length > 8192) throw invalid();
  let record: Record<string, unknown>;
  try {
    if (data.trimStart().startsWith('{')) {
      record = JSON.parse(data);
      if (!record || record.type !== 'ion-pairing' || record.version !== 1)
        throw invalid();
    } else {
      const url = new URL(data);
      if (
        !['ion:', 'waku:'].includes(url.protocol) ||
        url.hostname !== 'pair' ||
        !['', '/'].includes(url.pathname) ||
        url.username ||
        url.password ||
        url.port ||
        url.hash
      )
        throw invalid();
      for (const key of ['address', 'token', 'name']) {
        if (url.searchParams.getAll(key).length > 1) throw invalid();
      }
      record = Object.fromEntries(url.searchParams);
    }
    if (
      typeof record.address !== 'string' ||
      typeof record.token !== 'string' ||
      (record.name !== undefined && typeof record.name !== 'string')
    )
      throw invalid();
    const address = normalizeDaemonAddress(record.address);
    const original = new URL(record.address);
    // Do not silently discard credentials, paths or query strings from a scanned address.
    if (
      !['ws:', 'wss:'].includes(original.protocol) ||
      !['', '/'].includes(original.pathname) ||
      original.search ||
      original.hash ||
      original.username ||
      original.password ||
      (!address.startsWith('wss://') && !isPrivateDaemonAddress(address))
    )
      throw invalid();
    const token = record.token.trim();
    const name = (record.name as string | undefined)?.trim() ?? '';
    if (
      !token ||
      token.length > 4096 ||
      /[\u0000-\u001f\u007f]/.test(token) ||
      name.length > 80 ||
      /[\u0000-\u001f\u007f]/.test(name)
    )
      throw invalid();
    return { address, token, name };
  } catch {
    // Never reflect the scanned content: it contains the desktop's access token.
    throw invalid();
  }
}
