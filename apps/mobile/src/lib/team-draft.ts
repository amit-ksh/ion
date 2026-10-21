import type { TeamMember } from '@waku/client';

/** Mirrors the existing daemon's team invariants before submitting a draft. */
export function teamDraftProblem(members: TeamMember[], availableAgents: ReadonlySet<string>): string | null {
  if (members.length < 1 || members.length > 32) return 'Choose 1–32 agents.';
  const parents = new Map(members.map(member => [member.agentId, member.parentId]));
  if (parents.size !== members.length) return 'Each agent can appear only once.';
  if (members.filter(member => member.parentId === null).length !== 1) return 'Choose exactly one team lead.';
  for (const member of members) {
    if (!availableAgents.has(member.agentId)) return 'An agent is unavailable. Remove it or refresh.';
    const seen = new Set<string>();
    let id: string | null = member.agentId;
    while (id !== null) {
      if (seen.has(id)) return 'This reporting structure contains a cycle.';
      seen.add(id);
      if (!parents.has(id)) return 'Every manager must belong to this team.';
      id = parents.get(id)!;
    }
  }
  return null;
}
