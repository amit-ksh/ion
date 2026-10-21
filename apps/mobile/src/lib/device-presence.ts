/** A bounded, printable client-reported label, not a hardware attestation. */
export function mobileDeviceLabel(platform: string, constants: unknown): string {
  if (platform !== 'android') return 'Mobile device';
  const model = constants && typeof constants === 'object' && 'Model' in constants
    ? constants.Model : undefined;
  if (typeof model !== 'string') return 'Android';
  const label = model.replace(/[^\x20-\x7e]/g, '').trim().slice(0, 64).trim();
  return label || 'Android';
}
