import type { AgentProfile, ControlSnapshot, ProviderKind, Team } from '@waku/client';

// Mobile controls the user's paired desktop, which runs every harness,
// including Nebius inference and Nebius sandbox compute for team agents.
// Shared/gateway destinations stay unavailable in this app.
export const MOBILE_CLOUD_SERVICES = false;
export const MOBILE_HARNESSES: ProviderKind[] = [
  'codex', 'claude', 'openCode', 'openCode2', 'amp', 'antigravity', 'cursor',
  'deepSeek', 'fx', 'grok', 'kimi', 'nebius', 'ohMyPi', 'pi',
];
export function mobileHarness(provider: ProviderKind | null): provider is ProviderKind {
  return provider !== null && MOBILE_HARNESSES.includes(provider);
}
export function assertMobileProfiles(profiles: Pick<AgentProfile, 'provider' | 'execution'>[] | undefined) {
  if (!profiles?.length) throw new Error('Could not verify the team. Refresh before sending.');
  // The daemon checks sandbox readiness itself, exactly as for desktop runs.
  if (profiles.some(profile => !mobileHarness(profile.provider)))
    throw new Error('This team uses a harness this app does not offer. Run it from desktop.');
}
export function assertMobileTeam(team: Team, snapshot: ControlSnapshot) {
  const agents = snapshot.records.filter((record): record is AgentProfile & { kind: 'agent' } => record.kind === 'agent');
  const profiles = team.members.map(member => agents.find(agent => agent.id === member.agentId));
  if (profiles.some(profile => !profile)) throw new Error('Team agent unavailable. Refresh before sending.');
  assertMobileProfiles(profiles as AgentProfile[]);
}
