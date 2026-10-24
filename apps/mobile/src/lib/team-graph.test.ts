import { expect, test } from 'bun:test';
import type { TeamMember } from '@waku/client';
import { teamDraftProblem } from './team-draft';
import { TEAM_NODE_HEIGHT, TEAM_NODE_WIDTH, addTeamMember, removeTeamMember, reparentTeamMember, teamGraphLayout, teamParentAllowed } from './team-graph';

const team: TeamMember[] = [
  { agentId: 'lead', parentId: null },
  { agentId: 'marketing', parentId: 'lead' },
  { agentId: 'technical', parentId: 'lead' },
  { agentId: 'operations', parentId: 'lead' },
  { agentId: 'frontend', parentId: 'technical' },
  { agentId: 'backend', parentId: 'technical' },
];
const available = new Set(team.map(m => m.agentId));

test('reference hierarchy retains identities and branches without overlapping nodes', () => {
  const graph = teamGraphLayout(team);
  expect(graph.problem).toBeNull();
  expect(new Set(graph.nodes.map(n => n.id))).toEqual(available);
  for (const a of graph.nodes) for (const b of graph.nodes) if (a.id !== b.id && a.top === b.top && a.left < b.left)
    expect(a.left + TEAM_NODE_WIDTH).toBeLessThanOrEqual(b.left);
  const manager = graph.nodes.find(n => n.id === 'technical')!;
  for (const id of ['frontend', 'backend']) expect(graph.nodes.find(n => n.id === id)!.top).toBeGreaterThan(manager.top);
  for (const line of graph.lines) {
    expect(line.left).toBeGreaterThanOrEqual(0);
    expect(line.top).toBeGreaterThanOrEqual(0);
    expect(line.width).toBeGreaterThan(0);
    expect(line.height).toBeGreaterThan(0);
    expect(line.left + line.width).toBeLessThanOrEqual(graph.width);
    expect(line.top + line.height).toBeLessThanOrEqual(graph.height);
  }
});
test('empty and single-member graphs have bounded geometry', () => {
  expect(teamGraphLayout([])).toMatchObject({ width: 0, height: 0, nodes: [], problem: null });
  expect(teamGraphLayout([team[0]])).toMatchObject({ width: TEAM_NODE_WIDTH, height: TEAM_NODE_HEIGHT, lines: [] });
});
test('larger accessible text keeps node rows and connectors apart', () => {
  const width = TEAM_NODE_WIDTH * 2;
  const height = TEAM_NODE_HEIGHT * 2;
  const graph = teamGraphLayout(team, { width, height });
  expect(graph.problem).toBeNull();
  for (const node of graph.nodes) {
    const parentId = team.find(m => m.agentId === node.id)?.parentId;
    if (parentId) expect(node.top).toBeGreaterThanOrEqual(graph.nodes.find(n => n.id === parentId)!.top + height);
    expect(node.left + width).toBeLessThanOrEqual(graph.width);
  }
  for (const line of graph.lines) expect(line.top + line.height).toBeLessThanOrEqual(graph.height);
});
test('invalid legacy cycles and missing reporting agents do not draw a misleading graph', () => {
  const cycle = [team[0], { agentId: 'a', parentId: 'b' }, { agentId: 'b', parentId: 'a' }];
  expect(teamGraphLayout(cycle)).toMatchObject({ nodes: [], problem: expect.stringContaining('cycle') });
  expect(teamGraphLayout([{ ...team[0], parentId: 'missing' }]).problem).not.toBeNull();
});
test('manager cannot report to itself, a descendant or an unavailable agent', () => {
  for (const id of ['lead', 'frontend', 'missing']) expect(teamParentAllowed(team, 'lead', id)).toBe(false);
  expect(reparentTeamMember(team, 'technical', 'frontend')).toBe(team);
  expect(reparentTeamMember(team, 'missing', null)).toBe(team);
});
test('promoting a descendant preserves a valid complete hierarchy and the original draft', () => {
  const updated = reparentTeamMember(team, 'frontend', null);
  expect(teamDraftProblem(updated, available)).toBeNull();
  expect(updated).toHaveLength(team.length);
  expect(updated.find(m => m.agentId === 'lead')?.parentId).toBe('frontend');
  expect(team[0].parentId).toBeNull();
  expect(teamGraphLayout(updated).problem).toBeNull();
});
test('removing a manager retains and reparents reports; removing the lead retains deeper branches', () => {
  const updated = removeTeamMember(team, 'technical');
  expect(teamDraftProblem(updated, available)).toBeNull();
  expect(updated.find(m => m.agentId === 'frontend')?.parentId).toBe('lead');
  expect(updated.find(m => m.agentId === 'backend')?.parentId).toBe('lead');
  const noLead = removeTeamMember(team, 'lead');
  expect(teamDraftProblem(noLead, available)).toBeNull();
  expect(noLead.find(m => m.agentId === 'frontend')?.parentId).toBe('technical');
  expect(removeTeamMember(team, 'missing')).toBe(team);
});
test('new direct reports use the selected manager, with duplicate and stale choices bounded', () => {
  const updated = addTeamMember(team, 'reviewer', 'technical');
  expect(updated.at(-1)).toEqual({ agentId: 'reviewer', parentId: 'technical' });
  expect(teamDraftProblem(updated, new Set([...available, 'reviewer']))).toBeNull();
  expect(addTeamMember(updated, 'reviewer')).toBe(updated);
  expect(addTeamMember(team, 'reviewer', 'missing').at(-1)?.parentId).toBe('lead');
  expect(addTeamMember([], 'first')).toEqual([{ agentId: 'first', parentId: null }]);
  const full = Array.from({ length: 32 }, (_, i) => ({ agentId: String(i), parentId: i ? '0' : null }));
  expect(addTeamMember(full, 'extra')).toBe(full);
  expect(teamGraphLayout([...full, { agentId: 'extra', parentId: '0' }]).nodes).toEqual([]);
});
