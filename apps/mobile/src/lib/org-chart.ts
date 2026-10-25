import type { AgentProfile, Team, TeamMember } from '@waku/client';
import { addTeamMember, teamGraphLayout } from './team-graph';

/** Base card geometry in dp; callers scale it for large text. */
export const ORG_NODE = { width: 200, height: 84 };
export const ORG_CHIP = { width: 288, height: 104 };
/** The 1.5dp line from a team's header chip down to its lead. */
const LINK = 36;
/** Room under a card for its centered "+" button. */
const OVERHANG = 18;
const GAP_X = 56;
const GAP_Y = 64;
const GRID_GAP_X = 24;
const GRID_GAP_Y = 44;
const PAD = 48;
const MAX_TEAM_SIZE = 32;
const STROKE = 1.5;

function vertical(x: number, from: number, to: number): OrgSegment {
  return { left: x - STROKE / 2, top: Math.min(from, to), width: STROKE, height: Math.abs(to - from) };
}
function horizontal(from: number, to: number, y: number): OrgSegment {
  return { left: Math.min(from, to) - STROKE / 2, top: y - STROKE / 2, width: Math.abs(to - from) + STROKE, height: STROKE };
}

export type OrgSegment = { left: number; top: number; width: number; height: number };
/** A connector drawn as thin rectangles (no SVG: the canvas can be far larger than the screen). */
export type OrgEdge = { key: string; segments: OrgSegment[] };
export type OrgNode = { key: string; agentId: string; teamId: string | null; lead: boolean; left: number; top: number };
export type OrgCluster = {
  key: string;
  team: Team | null;
  /** Agents in this cluster (the team's members, or every agent on no team). */
  count: number;
  left: number; top: number; width: number; height: number;
  chip: { left: number; top: number; width: number; height: number };
  problem: string | null;
};
export type OrgChartLayout = {
  clusters: OrgCluster[];
  nodes: OrgNode[];
  /** Orthogonal connectors (chip to lead, parent to report). */
  edges: OrgEdge[];
  width: number;
  height: number;
};

/**
 * Every agent and its reporting lines on one canvas. Each team is a cluster
 * (header chip, a line to the lead, the lead's tree below with elbow
 * connectors); clusters sit left to right and wrap. Agents on no team close
 * the canvas as a grid under a "Not on a team" chip.
 */
export function orgChartLayout(
  teams: readonly Team[],
  agents: readonly AgentProfile[],
  scale = 1,
  wrapWidth = 1100,
): OrgChartLayout {
  const node = { width: Math.ceil(ORG_NODE.width * scale), height: Math.ceil(ORG_NODE.height * scale) };
  const chip = { width: Math.ceil(ORG_CHIP.width * scale), height: Math.ceil(ORG_CHIP.height * scale) };
  type Draft = { cluster: Omit<OrgCluster, 'left' | 'top'>; nodes: Omit<OrgNode, 'left' | 'top'>[]; place: (left: number, top: number) => { nodes: OrgNode[]; edges: OrgEdge[] } };
  const drafts: Draft[] = [];
  const charted = new Set<string>();
  for (const team of teams) {
    for (const member of team.members) charted.add(member.agentId);
    const tree = teamGraphLayout(team.members, node);
    const width = Math.max(tree.width, chip.width);
    const treeLeft = (width - tree.width) / 2;
    const chipLeft = (width - chip.width) / 2;
    const treeTop = chip.height + LINK;
    const parents = new Map(team.members.map(m => [m.agentId, m.parentId]));
    const height = tree.nodes.length ? treeTop + tree.height + OVERHANG : chip.height;
    drafts.push({
      cluster: { key: team.id, team, count: team.members.length, width, height, problem: tree.problem,
        chip: { left: chipLeft, top: 0, width: chip.width, height: chip.height } },
      nodes: [],
      place: (left, top) => {
        const nodes = tree.nodes.map(n => ({ key: `${team.id}:${n.id}`, agentId: n.id, teamId: team.id, lead: parents.get(n.id) === null,
          left: left + treeLeft + n.left, top: top + treeTop + n.top }));
        const positions = new Map(nodes.map(n => [n.agentId, n]));
        const edges: OrgEdge[] = [];
        const lead = nodes.find(n => n.lead);
        if (lead) {
          const x = lead.left + node.width / 2;
          edges.push({ key: `${team.id}:chip`, segments: [vertical(x, top + chip.height, lead.top)] });
        }
        for (const child of nodes) {
          const parent = positions.get(parents.get(child.agentId) ?? '');
          if (!parent) continue;
          const x1 = parent.left + node.width / 2;
          const y1 = parent.top + node.height;
          const x2 = child.left + node.width / 2;
          const y2 = child.top;
          const mid = (y1 + y2) / 2;
          edges.push({ key: child.key, segments: [vertical(x1, y1, mid), horizontal(x1, x2, mid), vertical(x2, mid, y2)] });
        }
        return { nodes, edges };
      },
    });
  }
  const loose = agents.filter(agent => !charted.has(agent.id));
  if (loose.length) {
    const columns = Math.min(2, loose.length);
    const rows = Math.ceil(loose.length / columns);
    const gridWidth = columns * node.width + (columns - 1) * GRID_GAP_X;
    const width = Math.max(gridWidth, chip.width);
    const gridLeft = (width - gridWidth) / 2;
    const gridTop = chip.height + LINK;
    drafts.push({
      cluster: { key: 'loose', team: null, count: loose.length, width,
        height: gridTop + rows * node.height + (rows - 1) * GRID_GAP_Y + OVERHANG, problem: null,
        chip: { left: (width - chip.width) / 2, top: 0, width: chip.width, height: chip.height } },
      nodes: [],
      place: (left, top) => ({
        nodes: loose.map((agent, index) => ({ key: `loose:${agent.id}`, agentId: agent.id, teamId: null, lead: false,
          left: left + gridLeft + (index % columns) * (node.width + GRID_GAP_X),
          top: top + gridTop + Math.floor(index / columns) * (node.height + GRID_GAP_Y) })),
        edges: [],
      }),
    });
  }
  const wrap = Math.max(wrapWidth, ...drafts.map(d => d.cluster.width));
  const result: OrgChartLayout = { clusters: [], nodes: [], edges: [], width: 0, height: 0 };
  let x = 0;
  let y = 0;
  let row = 0;
  for (const draft of drafts) {
    if (x > 0 && x + draft.cluster.width > wrap) { x = 0; y += row + GAP_Y; row = 0; }
    const left = PAD + x;
    const top = PAD + y;
    const placed = draft.place(left, top);
    result.clusters.push({ ...draft.cluster, left, top,
      chip: { ...draft.cluster.chip, left: left + draft.cluster.chip.left, top } });
    result.nodes.push(...placed.nodes);
    result.edges.push(...placed.edges);
    result.width = Math.max(result.width, left + draft.cluster.width + PAD);
    result.height = Math.max(result.height, top + draft.cluster.height + PAD);
    x += draft.cluster.width + GAP_X;
    row = Math.max(row, draft.cluster.height);
  }
  return result;
}

/**
 * The team write for a new or existing agent placed under `parentId`: it
 * joins the parent's team, or, when the parent is on no team, a new
 * "<Parent>'s team" is created with the parent as lead. Null when the team is
 * full or the agent is already a member.
 */
export function teamWithReport(input: {
  team: Team | null;
  parent: Pick<AgentProfile, 'id' | 'name'>;
  childId: string;
  newTeamId: string;
}): (Team & { kind: 'team' }) | null {
  if (input.team) {
    const members = addTeamMember(input.team.members, input.childId, input.parent.id);
    if (members === input.team.members || !members.some(m => m.agentId === input.childId && m.parentId === input.parent.id)) return null;
    return { ...input.team, kind: 'team', members };
  }
  if (input.childId === input.parent.id) return null;
  const members: TeamMember[] = [{ agentId: input.parent.id, parentId: null }, { agentId: input.childId, parentId: input.parent.id }];
  return { kind: 'team', id: input.newTeamId, name: `${input.parent.name}'s team`, objective: '', members };
}

export function teamIsFull(team: Team): boolean {
  return team.members.length >= MAX_TEAM_SIZE;
}
