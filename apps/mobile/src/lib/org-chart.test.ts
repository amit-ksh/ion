import { expect, test } from 'bun:test';
import type { AgentProfile, Team } from '@waku/client';
import { ORG_CHIP, ORG_NODE, orgChartLayout, teamWithReport } from './org-chart';

const agent = (id: string): AgentProfile => ({
  id, name: id, expertise: '', instructions: id, provider: 'codex', model: null, expectedOutput: '',
  execution: { environment: 'local', sandboxTarget: null, localFallback: false },
  access: { workspaceRead: true, workspaceWrite: false, shell: false, network: false, appActions: [] }, maxParallelTasks: 1,
});
const agents = ['lead', 'ui', 'qa', 'native', 'docs', 'solo', 'spare'].map(agent);
const ship: Team = { id: 'ship', name: 'Ship', objective: '', members: [
  { agentId: 'lead', parentId: null }, { agentId: 'ui', parentId: 'lead' }, { agentId: 'qa', parentId: 'lead' }, { agentId: 'native', parentId: 'ui' }] };
const docs: Team = { id: 'docs', name: 'Docs', objective: '', members: [{ agentId: 'docs', parentId: null }] };

function overlaps(a: { left: number; top: number; width: number; height: number }, b: typeof a) {
  return a.left < b.left + b.width && b.left < a.left + a.width && a.top < b.top + b.height && b.top < a.top + a.height;
}

test('every agent is placed once per team plus a grid for agents on no team', () => {
  const chart = orgChartLayout([ship, docs], agents);
  expect(chart.clusters.map(c => c.key)).toEqual(['ship', 'docs', 'loose']);
  expect(chart.nodes.filter(n => n.teamId === 'ship').map(n => n.agentId).sort()).toEqual(['lead', 'native', 'qa', 'ui']);
  expect(chart.nodes.filter(n => n.teamId === null).map(n => n.agentId)).toEqual(['solo', 'spare']);
  expect(chart.nodes.filter(n => n.lead).map(n => n.agentId).sort()).toEqual(['docs', 'lead']);
});

test('clusters and cards never overlap and stay inside the canvas', () => {
  const chart = orgChartLayout([ship, docs], agents, 1, 600);
  for (const a of chart.clusters) for (const b of chart.clusters) if (a !== b) expect(overlaps(a, b)).toBe(false);
  const cards = chart.nodes.map(n => ({ left: n.left, top: n.top, width: ORG_NODE.width, height: ORG_NODE.height }));
  for (const a of cards) for (const b of cards) if (a !== b) expect(overlaps(a, b)).toBe(false);
  for (const card of cards) {
    expect(card.left + card.width).toBeLessThanOrEqual(chart.width);
    expect(card.top + card.height).toBeLessThanOrEqual(chart.height);
  }
  for (const segment of chart.edges.flatMap(edge => edge.segments)) {
    expect(segment.width).toBeGreaterThan(0);
    expect(segment.height).toBeGreaterThan(0);
    expect(segment.left + segment.width).toBeLessThanOrEqual(chart.width);
    expect(segment.top + segment.height).toBeLessThanOrEqual(chart.height);
  }
});

test('each team chip sits above its lead, joined by a vertical line', () => {
  const chart = orgChartLayout([ship], agents.slice(0, 4));
  const cluster = chart.clusters[0]!;
  const lead = chart.nodes.find(n => n.lead)!;
  expect(lead.top).toBeGreaterThan(cluster.chip.top + ORG_CHIP.height);
  expect(lead.left + ORG_NODE.width / 2).toBeCloseTo(cluster.chip.left + ORG_CHIP.width / 2, 5);
  expect(chart.edges.find(e => e.key === 'ship:chip')).toBeTruthy();
  // One elbow per report.
  expect(chart.edges.filter(e => e.key !== 'ship:chip')).toHaveLength(3);
});

test('narrow wrap widths stack clusters into rows', () => {
  const chart = orgChartLayout([ship, docs], agents, 1, 300);
  const [first, second] = chart.clusters;
  expect(second!.top).toBeGreaterThanOrEqual(first!.top + first!.height);
});

test('a subagent joins the parent team under the parent, or founds the parent team', () => {
  const joined = teamWithReport({ team: ship, parent: agent('qa'), childId: 'new', newTeamId: 't' })!;
  expect(joined.members.find(m => m.agentId === 'new')).toEqual({ agentId: 'new', parentId: 'qa' });
  expect(joined.members.filter(m => m.parentId === null)).toHaveLength(1);
  const founded = teamWithReport({ team: null, parent: agent('solo'), childId: 'new', newTeamId: 't' })!;
  expect(founded).toMatchObject({ id: 't', name: "solo's team", members: [{ agentId: 'solo', parentId: null }, { agentId: 'new', parentId: 'solo' }] });
  expect(teamWithReport({ team: ship, parent: agent('qa'), childId: 'ui', newTeamId: 't' })).toBeNull();
  const full: Team = { ...ship, members: Array.from({ length: 32 }, (_, i) => ({ agentId: `a${i}`, parentId: i ? 'a0' : null })) };
  expect(teamWithReport({ team: full, parent: agent('a0'), childId: 'new', newTeamId: 't' })).toBeNull();
});
