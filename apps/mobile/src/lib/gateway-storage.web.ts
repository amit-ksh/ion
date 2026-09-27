import type { GatewayPairing } from './gateway-client';

// Web has no native vault. Keep a pairing only while this tab is open.
let pairing: GatewayPairing | null = null;

export async function readGatewayPairing(): Promise<GatewayPairing | null> { return pairing; }
export async function writeGatewayPairing(value: GatewayPairing): Promise<void> { pairing = value; }
export async function deleteGatewayPairing(): Promise<void> { pairing = null; }
