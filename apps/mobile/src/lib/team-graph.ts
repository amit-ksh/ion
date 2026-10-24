import type { TeamMember } from '@waku/client';
import { teamDraftProblem } from './team-draft';

export const TEAM_NODE_WIDTH = 216;
export const TEAM_NODE_HEIGHT = 76;
const GAP = 24;
const LEVEL = 120;
export type TeamGraphLayout = {
  nodes: { id: string; left: number; top: number }[];
  lines: { left: number; top: number; width: number; height: number }[];
  width: number;
  height: number;
  problem: string | null;
};

/** Bounded in-memory layout; identities and reporting edges remain unchanged. */
export function teamGraphLayout(members: readonly TeamMember[], size = { width: TEAM_NODE_WIDTH, height: TEAM_NODE_HEIGHT }): TeamGraphLayout {
  const result: TeamGraphLayout = { nodes: [], lines: [], width: 0, height: 0, problem: null };
  if (!members.length) return result;
  result.problem = teamDraftProblem([...members], new Set(members.map(m => m.agentId)));
  if (result.problem) return result;
  const children = new Map<string, string[]>();
  for (const member of members) if (member.parentId !== null) {
    const reports = children.get(member.parentId) ?? [];
    reports.push(member.agentId);
    children.set(member.parentId, reports);
  }
  const widths = new Map<string, number>();
  function measure(id: string): number {
    const reports = children.get(id) ?? [];
    const width = Math.max(size.width,
      reports.reduce((sum, child) => sum + measure(child), 0) + Math.max(0, reports.length - 1) * GAP);
    widths.set(id, width);
    return width;
  }
  function place(id: string, left: number, depth: number) {
    const center = left + widths.get(id)! / 2;
    const level = size.height + LEVEL - TEAM_NODE_HEIGHT;
    const top = depth * level;
    result.nodes.push({ id, left: center - size.width / 2, top });
    result.height = Math.max(result.height, top + size.height);
    let cursor = left;
    const centers: number[] = [];
    for (const child of children.get(id) ?? []) {
      centers.push(cursor + widths.get(child)! / 2);
      place(child, cursor, depth + 1);
      cursor += widths.get(child)! + GAP;
    }
    if (!centers.length) return;
    const split = top + size.height + (level - size.height) / 2;
    result.lines.push({ left: center, top: top + size.height, width: 1, height: split - top - size.height });
    const first = Math.min(center, centers[0]);
    const last = Math.max(center, centers.at(-1)!);
    result.lines.push({ left: first, top: split, width: Math.max(1, last - first), height: 1 });
    for (const childCenter of centers) result.lines.push({ left: childCenter, top: split, width: 1, height: (depth + 1) * level - split });
  }
  const lead = members.find(m => m.parentId === null)!;
  result.width = measure(lead.agentId);
  place(lead.agentId, 0, 0);
  return result;
}

export function teamParentAllowed(members: readonly TeamMember[], id: string, parent: string): boolean {
  const parents = new Map(members.map(m => [m.agentId, m.parentId]));
  if (!parents.has(id)) return false;
  const seen = new Set<string>();
  let current: string | null = parent;
  while (current !== null) {
    if (current === id || seen.has(current) || !parents.has(current)) return false;
    seen.add(current);
    current = parents.get(current)!;
  }
  return true;
}

/** Stale choices are a no-op. Promoting a lead retains every existing member. */
export function reparentTeamMember(members: TeamMember[], id: string, parentId: string | null): TeamMember[] {
  if (!members.some(m => m.agentId === id) || parentId !== null && !teamParentAllowed(members, id, parentId)) return members;
  return members.map(m => m.agentId === id ? { ...m, parentId }
    : parentId === null && m.parentId === null ? { ...m, parentId: id } : m);
}

/** Removing a membership preserves direct reports and does not delete agents. */
export function removeTeamMember(members: TeamMember[], id: string): TeamMember[] {
  const removed = members.find(m => m.agentId === id);
  if (!removed) return members;
  const remaining = members.filter(m => m.agentId !== id);
  const replacement = removed.parentId ?? remaining.find(m => m.parentId === id)?.agentId ?? remaining[0]?.agentId ?? null;
  return remaining.map(m => removed.parentId === null && m.agentId === replacement ? { ...m, parentId: null }
    : m.parentId === id ? { ...m, parentId: replacement } : m);
}

export function addTeamMember(members: TeamMember[], id: string, parentId?: string | null): TeamMember[] {
  if (members.length >= 32 || members.some(m => m.agentId === id)) return members;
  const parent = members.some(m => m.agentId === parentId) ? parentId!
    : members.find(m => m.parentId === null)?.agentId ?? null;
  return [...members, { agentId: id, parentId: parent }];
}
