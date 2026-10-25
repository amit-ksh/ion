import type { AgentProfile, ControlRecord, ProviderKind, Team, TeamDecision, TeamRunSummary } from '@waku/client';
import * as Crypto from 'expo-crypto';
import { router } from 'expo-router';
import { useMemo, useRef, useState, type ReactNode } from 'react';
import { ActivityIndicator, Alert, FlatList, Pressable, StyleSheet, Text, View } from 'react-native';
import { AgentAvatar, type AgentPresence } from '@/components/agent-avatar';
import { ApprovalCard, type ApprovalAction } from '@/components/approval-card';
import { AppSymbol } from '@/components/app-symbol';
import { Chip } from '@/components/chip';
import { ConnectionBanner } from '@/components/connection-banner';
import { EntityRow, rowPosition, type RowPosition } from '@/components/entity-row';
import { OrgCanvas, type TeamAction } from '@/components/org-canvas';
import { ProviderIcon } from '@/components/provider-icon';
import { RemoteProjectPicker } from '@/components/remote-project-picker';
import { RunStatusIcon } from '@/components/run-row';
import { useScreenHeaderInset } from '@/components/screen-header';
import { SegmentedTabs } from '@/components/segmented-tabs';
import { Sheet, SheetRow } from '@/components/sheet';
import { StatusPill, TicketCard, ticketStatus, type TicketStatus } from '@/components/ticket-card';
import { WorkbenchButton, WorkbenchCard, WorkbenchRow, WorkbenchState } from '@/components/workbench';
import { agentAdapterLabel } from '@/components/team-graph';
import { MonoFont, Radius } from '@/constants/theme';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useProviderCatalog, useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { displaySessionTitle, providerLabel, relativeSessionTime, sessionTimestamp } from '@/lib/session-presentation';
import { useRuntime } from '@/lib/runtime-context';
import { teamIsFull, teamWithReport, type OrgNode } from '@/lib/org-chart';
import { removeTeamMember } from '@/lib/team-graph';
import { sessionTicketState, ticketNumber } from '@/lib/ticket';
import { relativeTimeLabel } from '@/lib/time-labels';

const sections = ['Tasks', 'Projects', 'Teams', 'Decisions'] as const;
type Section = typeof sections[number];
const teamViews = ['Org chart', 'List', 'Runs'] as const;
type TeamView = typeof teamViews[number];
type SymbolName = Parameters<typeof AppSymbol>[0]['name'];
type TeamRecord = Team & { kind: 'team' };
type AgentRecord = AgentProfile & { kind: 'agent' };
type Ticket = {
  kind: 'ticket'; id: string; title: string; station: string; open: () => void;
  mark?: ReactNode; status?: TicketStatus; needsYou?: boolean; time?: string | null;
};
type Approval = {
  kind: 'approval'; id: string; type: string; icon: SymbolName; title: string; time: string | null;
  requester: { name: string; provider?: ProviderKind | null } | null; detail: string | null;
  approve: ApprovalAction | null; reject: ApprovalAction | null; open: () => void;
};
type Item =
  | Ticket
  | Approval
  | { kind: 'heading'; id: string; title: string; count: string }
  | { kind: 'member'; id: string; node: OrgNode; depth: number; position: RowPosition }
  | { kind: 'run'; id: string; run: TeamRunSummary; position: RowPosition };

const PENDING: TicketStatus = { label: 'Pending', icon: { ios: 'clock', android: 'schedule', web: 'schedule' }, tone: 'accent' };

function Mark({ name }: { name: 'folder' }) {
  const theme = useTheme();
  return <AppSymbol name={{ android: name, web: name }} size={15} tintColor={theme.textSecondary} />;
}

function compactTokens(tokens: number): string {
  if (tokens >= 1_000_000) return `${Number((tokens / 1_000_000).toFixed(1))}M`;
  if (tokens >= 1_000) return `${Math.round(tokens / 1_000)}k`;
  return String(tokens);
}

function teamRunState(run: TeamRunSummary): { icon: 'running' | 'waitingDecision' | 'blocked' | 'canceled' | 'completed'; status: TicketStatus } {
  if (run.status === 'active' && (run.waiting > 0 || run.needsAttention > 0)) return { icon: 'waitingDecision', status: ticketStatus('needsYou') };
  if (run.status === 'active') return { icon: 'running', status: { ...ticketStatus('working'), label: run.running > 0 ? `${run.running} running` : 'Working' } };
  if (run.status === 'stopping') return { icon: 'blocked', status: { label: 'Stopping', icon: { android: 'stop_circle', web: 'stop_circle' }, tone: 'warning' } };
  if (run.status === 'canceled') return { icon: 'canceled', status: { label: 'Stopped', icon: { android: 'block', web: 'block' }, tone: 'neutral' } };
  return { icon: 'completed', status: { ...ticketStatus('ready'), label: 'Completed' } };
}

export default function WorkScreen() {
  const theme = useTheme();
  const header = useScreenHeaderInset();
  const tasks = useTaskState();
  const runtime = useRuntime();
  const catalog = useProviderCatalog();
  const [section, setSection] = useState<Section>('Tasks');
  const [teamView, setTeamView] = useState<TeamView>('Org chart');
  const [runTeam, setRunTeam] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ node: OrgNode; page: 'menu' | 'existing' } | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [projectPicker, setProjectPicker] = useState(false);
  const [after, setAfter] = useState<string | null>(null);
  const [previous, setPrevious] = useState<(string | null)[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const locked = useRef(false);
  const control = useAutomationQuery(['workbench', section, after], async client => {
    const response = await client.request({ type: 'loadWorkspaceControl' });
    if (response.type !== 'workspaceControl') throw new Error('Could not load this workspace. Pull to refresh.');
    let decisions: TeamDecision[] = [];
    let runs: TeamRunSummary[] = [];
    let next: string | null = null;
    if (section === 'Decisions') {
      const page = await client.request({ type: 'listTeamDecisions', pendingOnly: true, projectId: null, after, limit: 50 });
      if (page.type !== 'teamDecisions') throw new Error('Could not load decisions. Pull to refresh.');
      decisions = page.page.decisions;
      next = page.page.nextCursor;
    }
    if (section === 'Teams') {
      // Runs only feed the Runs view; a daemon without them still shows every team.
      const page = await client.request({ type: 'listTeamRuns', projectId: null, teamId: null, after: null, limit: 32 }).catch(() => null);
      if (page?.type === 'teamRuns') runs = page.page.runs;
    }
    return { snapshot: response.snapshot, decisions, runs, next };
  });
  const records = control.data?.snapshot.records;
  const teamAgents = useMemo(() => (records ?? []).filter((r): r is AgentRecord => r.kind === 'agent'), [records]);
  const teams = useMemo(() => (records ?? []).filter((r): r is TeamRecord => r.kind === 'team'), [records]);
  const profiles = useMemo(() => new Map(teamAgents.map(agent => [agent.id, agent])), [teamAgents]);
  const owner = control.daemon.activeProfile?.id;
  const ready = control.fresh && Boolean(control.daemon.client) && busy === null;

  function presence(agentId: string): AgentPresence {
    const agent = profiles.get(agentId);
    if (!agent) return 'missing';
    const harness = catalog.providers.find(provider => provider.id === agent.provider);
    if (!harness) return 'missing';
    if (harness.isPending) return 'idle';
    return harness.installed ? 'ready' : 'missing';
  }

  async function settle(id: string, operation: () => Promise<void>) {
    if (!control.fresh || locked.current) return;
    locked.current = true; setBusy(id); setFeedback(null);
    try { await operation(); }
    catch (cause) { setFeedback(cause instanceof Error ? cause.message : String(cause)); }
    finally {
      locked.current = false; setBusy(null);
      void control.query.refetch(); void tasks.refetch();
    }
  }
  function resolveDecision(decision: TeamDecision, approve: boolean) {
    const client = control.daemon.client;
    if (!client) return;
    void settle(decision.id, async () => {
      const response = await client.request({ type: 'resolveTeamDecision', decisionId: decision.id, expectedRevision: decision.revision, approve });
      if (response.type !== 'teamDecision') throw new Error('The decision was not confirmed. Refresh before retrying.');
    });
  }
  function resolveAction(action: Extract<ControlRecord, { kind: 'action' }>, approve: boolean) {
    const client = control.daemon.client;
    const revision = control.data?.snapshot.revision;
    if (!client || revision === undefined) return;
    void settle(action.id, async () => {
      if (approve && !tasks.data?.projects.some(p => p.id === action.projectId)) throw new Error('The proposal’s project is unavailable. Refresh or restore the project before accepting.');
      const response = await client.request({ type: 'saveWorkspaceRecord', expectedRevision: revision, record: { ...action, kind: 'action', status: approve ? 'accepted' : 'declined' } });
      if (response.type !== 'workspaceControl') throw new Error('The proposal update was not confirmed. Refresh before retrying.');
      // An accepted task proposal is prepared from its review page, as before.
      if (approve && action.actionKind === 'proposedTask') router.push({ pathname: '/inbox-source', params: { source: 'workspaceAction', id: action.id, owner } });
    });
  }
  /** Revision-checked team write from the org chart (membership changes only). */
  function writeTeam(record: TeamRecord) {
    const client = control.daemon.client;
    const revision = control.data?.snapshot.revision;
    if (!client || revision === undefined) return;
    void settle(record.id, async () => {
      const response = await client.request({ type: 'saveWorkspaceRecord', expectedRevision: revision, record });
      if (response.type !== 'workspaceControl') throw new Error('The team update was not confirmed. Refresh before retrying.');
    });
  }
  function addExisting(node: OrgNode, childId: string) {
    const parent = profiles.get(node.agentId);
    const team = node.teamId ? teams.find(t => t.id === node.teamId) ?? null : null;
    if (!parent) return;
    const record = teamWithReport({ team, parent, childId, newTeamId: Crypto.randomUUID() });
    if (!record) { setFeedback('That team is full (32 agents) or already includes this agent.'); return; }
    writeTeam(record);
  }
  function removeFromTeam(node: OrgNode) {
    const team = teams.find(t => t.id === node.teamId);
    const agent = profiles.get(node.agentId);
    if (!team || node.lead) return;
    Alert.alert(`Remove ${agent?.name ?? 'this agent'} from ${team.name}?`, 'Its reports move up to its manager. The agent itself is kept.', [
      { text: 'Cancel', style: 'cancel' },
      { text: 'Remove', style: 'destructive', onPress: () => writeTeam({ ...team, kind: 'team', members: removeTeamMember(team.members, node.agentId) }) },
    ]);
  }
  function teamAction(team: Team, action: TeamAction) {
    if (action === 'task') router.push({ pathname: '/new-task', params: { team: team.id } });
    else if (action === 'runs') { setRunTeam(team.id); setTeamView('Runs'); }
    else router.push({ pathname: '/team-editor', params: { id: team.id } });
  }

  const items = ((): Item[] => {
    const now = Date.now();
    const projects = new Map(tasks.data?.projects.map(project => [project.id, project.name]));
    const pending = (id: string) => Boolean(runtime.permissions[id] || runtime.userInputs[id]);
    if (section === 'Tasks') return (tasks.data?.sessions ?? []).map<Ticket>(session => {
      const state = pending(session.id) ? 'needsYou' : sessionTicketState(session);
      return {
        kind: 'ticket', id: session.id, title: displaySessionTitle(session),
        station: projects.get(session.project_id) ?? 'Project',
        mark: <ProviderIcon provider={session.provider} size={14} />,
        status: ticketStatus(state), needsYou: state === 'needsYou',
        time: relativeSessionTime(sessionTimestamp(session), now),
        open: () => router.push({ pathname: '/session/[id]', params: { id: session.id } }),
      };
    }).sort((a, b) => Number(Boolean(b.needsYou)) - Number(Boolean(a.needsYou)));
    if (section === 'Projects') return (tasks.data?.projects ?? []).map<Ticket>(project => ({
      kind: 'ticket', id: project.id, title: project.name, station: project.path, mark: <Mark name="folder" />,
      open: () => router.push({ pathname: '/new-task', params: { project: project.id } }),
    }));
    if (section === 'Teams' && teamView === 'Runs') {
      const runs = (control.data?.runs ?? []).filter(run => !runTeam || run.teamId === runTeam);
      return runs.map<Item>((run, index) => ({ kind: 'run', id: run.id, run, position: rowPosition(index, runs.length) }));
    }
    if (section === 'Teams') {
      const result: Item[] = [];
      const charted = new Set<string>();
      for (const team of teams) {
        const parents = new Map(team.members.map(m => [m.agentId, m.parentId]));
        const depth = (id: string) => { let d = 0; let at = parents.get(id) ?? null; while (at && d < 32) { d++; at = parents.get(at) ?? null; } return d; };
        // Lead first, then each manager followed by its reports.
        const ordered: string[] = [];
        const visit = (id: string) => { if (ordered.includes(id)) return; ordered.push(id); for (const m of team.members) if (m.parentId === id) visit(m.agentId); };
        const lead = team.members.find(m => m.parentId === null);
        if (lead) visit(lead.agentId);
        for (const m of team.members) if (!ordered.includes(m.agentId)) ordered.push(m.agentId);
        result.push({ kind: 'heading', id: `heading:${team.id}`, title: team.name, count: team.members.length === 1 ? 'Lead only' : `Lead + ${team.members.length - 1}` });
        ordered.forEach((agentId, index) => {
          charted.add(agentId);
          result.push({ kind: 'member', id: `${team.id}:${agentId}`, depth: depth(agentId), position: rowPosition(index, ordered.length),
            node: { key: `${team.id}:${agentId}`, agentId, teamId: team.id, lead: parents.get(agentId) === null, left: 0, top: 0 } });
        });
      }
      const loose = teamAgents.filter(agent => !charted.has(agent.id));
      if (loose.length) result.push({ kind: 'heading', id: 'heading:loose', title: 'Not on a team', count: String(loose.length) });
      loose.forEach((agent, index) => result.push({ kind: 'member', id: `loose:${agent.id}`, depth: 0, position: rowPosition(index, loose.length),
        node: { key: `loose:${agent.id}`, agentId: agent.id, teamId: null, lead: false, left: 0, top: 0 } }));
      return result;
    }
    const review = (source: string, id: string, run?: string) => () => router.push({ pathname: '/inbox-source', params: { source, id, run, owner } });
    return [
      ...(!after ? (tasks.data?.sessions ?? []).filter(session => !session.team_assignment
        && (session.status === 'waiting' || pending(session.id))).map<Approval>(session => {
        const permission = runtime.permissions[session.id];
        const question = runtime.userInputs[session.id];
        const open = () => router.push({ pathname: '/session/[id]', params: { id: session.id } });
        const allow = permission?.options.find(option => option.allow);
        const deny = permission?.options.find(option => !option.allow);
        const respond = (optionId: string) => () => {
          if (!permission) return;
          void settle(session.id, () => runtime.respond(session.id, permission.requestId, optionId));
        };
        return {
          kind: 'approval', id: `session:${session.id}`, type: question ? 'Question' : 'Approval',
          icon: question ? { android: 'help', web: 'help' } : { android: 'verified_user', web: 'verified_user' },
          title: permission?.title || displaySessionTitle(session),
          time: relativeTimeLabel(sessionTimestamp(session), now),
          requester: { name: `${providerLabel(session.provider)} in ${projects.get(session.project_id) ?? 'Project'}`, provider: session.provider },
          detail: permission?.detail || null,
          approve: allow ? { label: allow.label, onPress: respond(allow.id), disabled: busy !== null }
            : { label: question ? 'Answer' : 'Review', onPress: open },
          reject: deny ? { label: deny.label, onPress: respond(deny.id), disabled: busy !== null } : null,
          open,
        };
      }) : []),
      ...(control.data?.decisions ?? []).map<Approval>(decision => {
        const agent = profiles.get(decision.agentId);
        return {
          kind: 'approval', id: decision.id, type: 'Decision', icon: { android: 'gavel', web: 'gavel' }, title: decision.title,
          time: relativeTimeLabel(decision.createdAt, now),
          requester: { name: agent?.name ?? 'Team agent', provider: agent?.provider ?? null },
          detail: decision.proposal || null,
          approve: { label: busy === decision.id ? 'Approving…' : 'Approve', onPress: () => resolveDecision(decision, true), disabled: !ready },
          reject: { label: 'Reject', onPress: () => resolveDecision(decision, false), disabled: !ready },
          open: review('teamDecision', decision.id, decision.runId),
        };
      }),
      ...(!after ? (records ?? []).filter((record): record is Extract<ControlRecord, { kind: 'action' }> => record.kind === 'action' && record.status === 'pending').map<Approval>(action => ({
        kind: 'approval', id: action.id, type: 'Proposal', icon: { android: 'lightbulb', web: 'lightbulb' }, title: action.title,
        time: relativeTimeLabel(action.createdAt, now),
        requester: { name: projects.get(action.projectId) ?? 'Project' },
        detail: action.detail || null,
        approve: { label: busy === action.id ? 'Accepting…' : 'Accept', onPress: () => resolveAction(action, true), disabled: !ready },
        reject: { label: 'Decline', onPress: () => resolveAction(action, false), disabled: !ready },
        open: review('workspaceAction', action.id),
      })) : []),
    ];
  })();
  const waiting = section === 'Decisions' ? items.length : 0;
  const error = control.query.error ?? tasks.error;
  const loading = !control.data && control.query.isFetching || !tasks.data && tasks.isFetching;
  const refresh = () => { void control.query.refetch(); void tasks.refetch(); };
  const disconnected = control.daemon.phase !== 'connected';

  const headerBlock = <View style={styles.header}>
    <ConnectionBanner />
    <SegmentedTabs tabs={sections} value={section} counts={{ Decisions: waiting }}
      onChange={item => { setSection(item); setAfter(null); setPrevious([]); setFeedback(null); }} />
    {section === 'Teams' && <View style={styles.teamBar}>
      <View style={styles.grow}>
        <SegmentedTabs tabs={teamViews} value={teamView} onChange={view => { setTeamView(view); if (view !== 'Runs') setRunTeam(null); }} />
      </View>
      <Pressable accessibilityRole="button" accessibilityLabel="Add an agent, a team or apps" onPress={() => setAddOpen(true)} style={styles.addTarget}>
        {({ pressed }) => <View collapsable={false} style={[styles.addDisc, { backgroundColor: theme.primary, opacity: pressed ? 0.85 : 1 }]}>
          <AppSymbol name={{ ios: 'plus', android: 'add', web: 'add' }} size={22} tintColor={theme.onPrimary} />
        </View>}
      </Pressable>
    </View>}
    {section !== 'Teams' && section !== 'Decisions' && <View style={styles.actions}>
      {section === 'Tasks' && <WorkbenchButton label="New task" primary icon={{ android: 'add', web: 'add' }} onPress={() => router.push('/new-task')} />}
      {section === 'Projects' && <WorkbenchButton label="Open folder" primary icon={{ android: 'create_new_folder', web: 'create_new_folder' }} onPress={() => setProjectPicker(true)} />}
    </View>}
    {section === 'Tasks' && <WorkbenchCard flush><WorkbenchRow label="Workflows & routines" icon={{ android: 'account_tree', web: 'account_tree' }} onPress={() => router.push('/automations')} /></WorkbenchCard>}
    {section === 'Teams' && teamView === 'Runs' && runTeam && <View style={styles.actions}>
      <Chip label={teams.find(t => t.id === runTeam)?.name ?? 'Team'} active chevron={false} icon={{ ios: 'xmark', android: 'close', web: 'close' }}
        accessibilityLabel="Show runs of every team" onPress={() => setRunTeam(null)} />
    </View>}
    {feedback && <Text accessibilityRole="alert" style={[styles.feedback, { color: theme.danger }]}>{feedback}</Text>}
    {error && <WorkbenchState icon={{ android: 'cloud_off', web: 'cloud_off' }} title="Could not refresh" detail={error instanceof Error ? error.message : 'Check your connection and retry.'}><WorkbenchButton label="Retry" onPress={refresh} /></WorkbenchState>}
  </View>;
  const emptyState = disconnected ? <WorkbenchState icon={{ android: 'link_off', web: 'link_off' }} title="Connect to your desktop" detail="Open Settings to reconnect or choose another paired desktop."><WorkbenchButton label="Open Settings" onPress={() => router.push('/settings')} /></WorkbenchState>
    : loading ? <ActivityIndicator accessibilityLabel={`Loading ${section.toLowerCase()}`} color={theme.accent} style={styles.loading} />
      : section === 'Teams' && teamView === 'Runs' ? <WorkbenchState icon={{ android: 'history', web: 'history' }} title="No team runs yet" detail="Runs appear here when a team takes on a task." />
        : <WorkbenchState
          icon={section === 'Decisions' ? { android: 'task_alt', web: 'task_alt' } : section === 'Teams' ? { android: 'groups', web: 'groups' } : section === 'Projects' ? { android: 'folder_open', web: 'folder_open' } : { android: 'receipt_long', web: 'receipt_long' }}
          title={section === 'Decisions' ? 'All caught up' : section === 'Teams' ? 'No agents yet' : `No ${section.toLowerCase()} yet`}
          detail={section === 'Decisions' ? 'Approvals and proposals that need your review appear here.' : section === 'Teams' ? 'Create an agent, then add subagents under it to form a team.' : section === 'Projects' ? 'Open a folder on your paired desktop.' : 'Start a task to work with an agent.'}>
          {section === 'Teams' && <WorkbenchButton label="New agent" primary icon={{ android: 'add', web: 'add' }} onPress={() => router.push('/agent-editor')} />}
        </WorkbenchState>;

  const sheets = <>
    <AgentMenuSheet menu={menu} teams={teams} agents={teamAgents} presence={presence}
      onPage={page => setMenu(current => current ? { ...current, page } : current)}
      onDismiss={() => setMenu(null)}
      onNewSubagent={node => { setMenu(null); router.push({ pathname: '/agent-editor', params: { parent: node.agentId, ...(node.teamId ? { team: node.teamId } : {}) } }); }}
      onAddExisting={(node, child) => { setMenu(null); addExisting(node, child); }}
      onEdit={node => { setMenu(null); router.push({ pathname: '/agent-editor', params: { id: node.agentId } }); }}
      onRemove={node => { setMenu(null); removeFromTeam(node); }} />
    <Sheet visible={addOpen} onDismiss={() => setAddOpen(false)} title="Add">
      <SheetRow label="New agent" description="An agent on no team until you add it under a lead" leading={<SheetIcon name="person_add" />}
        onPress={() => { setAddOpen(false); router.push('/agent-editor'); }} />
      <SheetRow label="New team" description="Pick a lead and the agents that report to it" leading={<SheetIcon name="group_add" />}
        onPress={() => { setAddOpen(false); router.push('/team-editor'); }} />
      <SheetRow label="Apps" description="Connect the accounts your agents use" leading={<SheetIcon name="extension" />}
        onPress={() => { setAddOpen(false); router.push('/apps'); }} />
    </Sheet>
  </>;

  if (section === 'Teams' && teamView === 'Org chart') {
    const chart = !disconnected && !loading && teamAgents.length > 0;
    return <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <View style={[styles.content, styles.fixedHeader, { paddingTop: header + 12 }]}>{headerBlock}</View>
      {chart ? <OrgCanvas teams={teams} agents={teamAgents} presence={presence}
        onOpenAgent={id => router.push({ pathname: '/agent-editor', params: { id } })}
        onAgentMenu={node => setMenu({ node, page: 'menu' })}
        onTeamAction={teamAction}
        onNewTeam={() => router.push('/team-editor')} /> : <View style={styles.content}>{emptyState}</View>}
      {sheets}
    </View>;
  }

  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <FlatList data={items} keyExtractor={item => item.id}
      contentContainerStyle={[styles.content, { paddingTop: header + 12 }]}
      refreshing={control.query.isFetching && Boolean(control.data)}
      onRefresh={refresh}
      ListHeaderComponent={headerBlock}
      ListEmptyComponent={emptyState}
      renderItem={({ item }) => {
        if (item.kind === 'heading') return <View style={styles.heading}>
          <Text accessibilityRole="header" numberOfLines={1} style={[styles.headingTitle, { color: theme.text }]}>{item.title}</Text>
          <Text style={[styles.mono, { color: theme.textTertiary }]}>{item.count}</Text>
        </View>;
        if (item.kind === 'member') {
          const agent = profiles.get(item.node.agentId);
          const name = agent?.name ?? 'Unavailable agent';
          return <EntityRow position={item.position}
            leading={<View style={{ marginLeft: Math.min(item.depth, 4) * 16 }}><AgentAvatar provider={agent?.provider} size={32} presence={presence(item.node.agentId)} /></View>}
            title={name}
            titleAccessory={item.node.lead ? <View style={[styles.lead, { backgroundColor: theme.primary }]}><Text style={[styles.leadLabel, { color: theme.onPrimary }]}>Lead</Text></View> : null}
            subtitle={agent ? agent.expertise || 'Agent' : 'Refresh or remove this agent'}
            meta={agent ? `${agent.model ?? 'Default model'} · ${agentAdapterLabel(agent)}` : null}
            accessibilityHint="Opens this agent"
            onPress={() => router.push({ pathname: '/agent-editor', params: { id: item.node.agentId } })}
            trailing={<Pressable accessibilityRole="button" accessibilityLabel={`Add under ${name}`} onPress={() => setMenu({ node: item.node, page: 'menu' })} style={styles.rowPlus}>
              {({ pressed }) => <View collapsable={false} style={[styles.rowPlusDisc, { backgroundColor: pressed ? theme.overlayStrong : theme.overlay, borderColor: theme.border }]}>
                <AppSymbol name={{ ios: 'plus', android: 'add', web: 'add' }} size={16} tintColor={theme.textSecondary} />
              </View>}
            </Pressable>} />;
        }
        if (item.kind === 'run') {
          const state = teamRunState(item.run);
          const done = item.run.assignments ? item.run.completed / item.run.assignments : 0;
          return <EntityRow position={item.position}
            leading={<RunStatusIcon status={state.icon} />}
            identifier={ticketNumber(item.run.id)} title={item.run.objective || item.run.teamName}
            subtitle={`${item.run.teamName} · ${relativeTimeLabel(item.run.updatedAt)}`}
            accessibilityLabel={`${item.run.objective || item.run.teamName}, ${state.status.label}`}
            accessibilityHint="Opens the run's lead task"
            onPress={() => router.push({ pathname: '/session/[id]', params: { id: item.run.rootTaskId } })}
            footer={<View style={styles.progressRow}>
              <StatusPill {...state.status} />
              <View style={[styles.track, { backgroundColor: theme.overlayStrong }]}>
                <View style={[styles.fill, { width: `${Math.round(done * 100)}%`, backgroundColor: theme.accent }]} />
              </View>
              <Text style={[styles.mono, { color: theme.textTertiary }]}>{item.run.completed}/{item.run.assignments} · {compactTokens(item.run.usage.consumedTokens)} tokens</Text>
            </View>} />;
        }
        if (item.kind === 'approval') return <View style={styles.item}>
          <ApprovalCard kind={item.type} icon={item.icon} requester={item.requester} title={item.title} time={item.time}
            status={PENDING} detail={item.detail} approve={item.approve} reject={item.reject} open={item.open} />
        </View>;
        return <View style={styles.item}>
          <TicketCard number={ticketNumber(item.id)} time={item.time} title={item.title}
            mark={item.mark} station={item.station} status={item.status} needsYou={item.needsYou} onPress={item.open}
            titleLines={section === 'Projects' ? 1 : 2} />
        </View>;
      }}
      ListFooterComponent={section === 'Decisions' ? <View style={styles.actions}>
        {previous.length > 0 && <WorkbenchButton label="Previous" onPress={() => { setAfter(previous.at(-1) ?? null); setPrevious(previous.slice(0, -1)); }} />}
        {control.data?.next && <WorkbenchButton label="Next" onPress={() => { setPrevious([...previous, after]); setAfter(control.data!.next); }} />}
      </View> : null} />
    <RemoteProjectPicker visible={projectPicker} onDismiss={() => setProjectPicker(false)} onSelect={() => { void tasks.refetch(); }} />
    {sheets}
  </View>;
}

function SheetIcon({ name }: { name: 'person_add' | 'group_add' | 'extension' | 'edit' | 'person_remove' | 'group' | 'chevron_left' }) {
  const theme = useTheme();
  return <View style={[styles.sheetIcon, { backgroundColor: theme.overlay }]}><AppSymbol name={{ android: name, web: name }} size={20} tintColor={theme.textSecondary} /></View>;
}

/** The "+" menu of an org chart card: subagents, membership and editing. */
function AgentMenuSheet({ menu, teams, agents, presence, onPage, onDismiss, onNewSubagent, onAddExisting, onEdit, onRemove }: {
  menu: { node: OrgNode; page: 'menu' | 'existing' } | null;
  teams: readonly TeamRecord[];
  agents: readonly AgentRecord[];
  presence: (agentId: string) => AgentPresence;
  onPage: (page: 'menu' | 'existing') => void;
  onDismiss: () => void;
  onNewSubagent: (node: OrgNode) => void;
  onAddExisting: (node: OrgNode, childId: string) => void;
  onEdit: (node: OrgNode) => void;
  onRemove: (node: OrgNode) => void;
}) {
  const theme = useTheme();
  const node = menu?.node;
  const agent = node ? agents.find(a => a.id === node.agentId) : undefined;
  const team = node?.teamId ? teams.find(t => t.id === node.teamId) : undefined;
  const name = agent?.name ?? 'This agent';
  const members = new Set(team?.members.map(m => m.agentId) ?? (node ? [node.agentId] : []));
  const candidates = agents.filter(a => !members.has(a.id));
  const full = team ? teamIsFull(team) : false;
  const manager = team && node && !node.lead ? agents.find(a => a.id === team.members.find(m => m.agentId === node.agentId)?.parentId) : undefined;
  const where = team ? node?.lead ? `Lead of ${team.name}` : `Reports to ${manager?.name ?? 'its manager'} in ${team.name}` : 'Not on a team';
  return <Sheet visible={Boolean(menu)} onDismiss={onDismiss} title={menu?.page === 'existing' ? `Add under ${name}` : name}>
    {node && menu?.page === 'existing' ? <>
      <SheetRow label="Back" leading={<SheetIcon name="chevron_left" />} onPress={() => onPage('menu')} />
      {candidates.length ? candidates.map(candidate => <SheetRow key={candidate.id} label={candidate.name}
        description={`${candidate.expertise || 'Agent'} · ${agentAdapterLabel(candidate)}`}
        leading={<AgentAvatar provider={candidate.provider} size={36} presence={presence(candidate.id)} />}
        onPress={() => onAddExisting(node, candidate.id)} />)
        : <Text style={[styles.sheetCaption, { color: theme.textSecondary }]}>Every agent is already here.</Text>}
    </> : node ? <>
      <Text style={[styles.sheetCaption, { color: theme.textSecondary }]}>{where}</Text>
      <SheetRow label="New subagent" description={full ? 'This team already has 32 agents' : team ? `Joins ${team.name} reporting to ${name}` : `Starts ${name}'s team with ${name} as lead`}
        disabled={full || !agent} leading={<SheetIcon name="person_add" />} onPress={() => onNewSubagent(node)} />
      <SheetRow label="Add existing agent" description={candidates.length ? `${candidates.length} available` : 'No other agents'}
        disabled={full || !agent || !candidates.length} leading={<SheetIcon name="group_add" />}
        trailing={<AppSymbol name={{ ios: 'chevron.right', android: 'chevron_right', web: 'chevron_right' }} size={20} tintColor={theme.textTertiary} />}
        onPress={() => onPage('existing')} />
      <SheetRow label="Edit agent" disabled={!agent} leading={<SheetIcon name="edit" />} onPress={() => onEdit(node)} />
      {team && !node.lead && <SheetRow label="Remove from team" destructive leading={<SheetIcon name="person_remove" />} onPress={() => onRemove(node)} />}
    </> : null}
  </Sheet>;
}

const styles = StyleSheet.create({
  screen: { flex: 1 }, content: { width: '100%', maxWidth: 1000, alignSelf: 'center', padding: 16, paddingBottom: 24 },
  fixedHeader: { paddingBottom: 0 },
  header: { gap: 12, marginBottom: 16 },
  teamBar: { alignItems: 'center', flexDirection: 'row', gap: 8 },
  grow: { flex: 1, minWidth: 0 },
  addTarget: { alignItems: 'center', height: 48, justifyContent: 'center', width: 48 },
  addDisc: { alignItems: 'center', borderRadius: 22, height: 44, justifyContent: 'center', width: 44 },
  actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8 }, item: { marginBottom: 12 }, loading: { padding: 32 },
  feedback: { fontSize: 14, lineHeight: 21 },
  heading: { alignItems: 'baseline', flexDirection: 'row', gap: 8, paddingBottom: 10, paddingHorizontal: 4, paddingTop: 8 },
  headingTitle: { flexShrink: 1, fontSize: 15, fontWeight: '600', lineHeight: 21 },
  mono: { fontFamily: MonoFont, fontSize: 12, fontVariant: ['tabular-nums'], lineHeight: 17 },
  lead: { borderRadius: Radius.pill, justifyContent: 'center', minHeight: 18, paddingHorizontal: 7 },
  leadLabel: { fontSize: 11, fontWeight: '700', lineHeight: 14 },
  rowPlus: { alignItems: 'center', height: 48, justifyContent: 'center', width: 48 },
  rowPlusDisc: { alignItems: 'center', borderRadius: 16, borderWidth: StyleSheet.hairlineWidth, height: 32, justifyContent: 'center', width: 32 },
  progressRow: { alignItems: 'center', flexDirection: 'row', gap: 10 },
  track: { borderRadius: 2, flex: 1, height: 4, overflow: 'hidden' },
  fill: { height: 4 },
  sheetIcon: { alignItems: 'center', borderRadius: 10, height: 36, justifyContent: 'center', width: 36 },
  sheetCaption: { fontSize: 14, lineHeight: 20, marginBottom: 8, marginHorizontal: 12, marginTop: -6 },
});
