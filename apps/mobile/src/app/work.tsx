import type { ControlRecord, TeamDecision } from '@waku/client';
import { router } from 'expo-router';
import { useMemo, useState } from 'react';
import { ActivityIndicator, FlatList, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { ConnectionBanner } from '@/components/connection-banner';
import { RemoteProjectPicker } from '@/components/remote-project-picker';
import { useScreenHeaderInset } from '@/components/screen-header';
import { WorkbenchButton, WorkbenchCard, WorkbenchRow, WorkbenchState } from '@/components/workbench';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { displaySessionTitle, providerLabel } from '@/lib/session-presentation';
import { useRuntime } from '@/lib/runtime-context';

const sections = ['Tasks', 'Projects', 'Teams', 'Decisions'] as const;
type Section = typeof sections[number];
type WorkRow = { id: string; title: string; detail: string; open: () => void; members?: string[] };

export default function WorkScreen() {
  const theme = useTheme();
  const header = useScreenHeaderInset();
  const tasks = useTaskState();
  const runtime = useRuntime();
  const [section, setSection] = useState<Section>('Tasks');
  const [projectPicker, setProjectPicker] = useState(false);
  const [after, setAfter] = useState<string | null>(null);
  const [previous, setPrevious] = useState<(string | null)[]>([]);
  const control = useAutomationQuery(['workbench', section, after], async client => {
    const response = await client.request({ type: 'loadWorkspaceControl' });
    if (response.type !== 'workspaceControl') throw new Error('Could not load this workspace. Pull to refresh.');
    let decisions: TeamDecision[] = [];
    let next: string | null = null;
    if (section === 'Decisions') {
      const page = await client.request({ type: 'listTeamDecisions', pendingOnly: true, projectId: null, after, limit: 50 });
      if (page.type !== 'teamDecisions') throw new Error('Could not load decisions. Pull to refresh.');
      decisions = page.page.decisions;
      next = page.page.nextCursor;
    }
    return { snapshot: response.snapshot, decisions, next };
  });
  const records = control.data?.snapshot.records;
  const owner = control.daemon.activeProfile?.id;
  const rows = useMemo<WorkRow[]>(() => {
    const projects = new Map(tasks.data?.projects.map(project => [project.id, project.name]));
    if (section === 'Tasks') return (tasks.data?.sessions ?? []).map(session => ({
      id: session.id, title: displaySessionTitle(session),
      detail: `${projects.get(session.project_id) ?? 'Project'} · ${providerLabel(session.provider)} · ${session.status}`,
      open: () => router.push({ pathname: '/session/[id]', params: { id: session.id } }),
    }));
    if (section === 'Projects') return (tasks.data?.projects ?? []).map(project => ({
      id: project.id, title: project.name, detail: project.path,
      open: () => router.push({ pathname: '/new-task', params: { project: project.id } }),
    }));
    if (section === 'Teams') {
      const agents = new Map((records ?? []).filter((record): record is Extract<ControlRecord, {kind: 'agent'}> => record.kind === 'agent').map(agent => [agent.id, agent]));
      return (records ?? []).flatMap(record => {
        if (record.kind === 'team') return [{ id: record.id, title: record.name,
          detail: record.objective || `${record.members.length} agents`,
          members: record.members.map(member => `${agents.get(member.agentId)?.name ?? 'Unavailable agent'} · ${member.parentId ? `Reports to ${agents.get(member.parentId)?.name ?? 'unavailable agent'}` : 'Team lead'}`),
          open: () => router.push({ pathname: '/team-editor', params: { id: record.id } }),
        }];
        if (record.kind === 'agent') return [{ id: record.id, title: record.name,
          detail: `${record.expertise || 'Agent'} · ${providerLabel(record.provider)} · ${record.execution.environment === 'local' ? 'Local' : 'Nebius'}`,
          open: () => router.push({ pathname: '/agent-editor', params: { id: record.id } }),
        }];
        return [];
      });
    }
    const review = (source: string, id: string, run?: string) => () => router.push({ pathname: '/inbox-source', params: { source, id, run, owner } });
    return [
      ...(!after ? (tasks.data?.sessions ?? []).filter(session => !session.team_assignment
        && (session.status === 'waiting' || runtime.permissions[session.id] || runtime.userInputs[session.id])).map(session => ({
        id: `session:${session.id}`, title: displaySessionTitle(session),
        detail: `${runtime.userInputs[session.id] ? 'Question' : 'Approval'} · ${projects.get(session.project_id) ?? 'Project'}`,
        open: () => router.push({ pathname: '/session/[id]', params: { id: session.id } }),
      })) : []),
      ...(control.data?.decisions ?? []).map(decision => ({ id: decision.id, title: decision.title,
        detail: `Decision · ${projects.get(decision.projectId) ?? 'Project'}`, open: review('teamDecision', decision.id, decision.runId) })),
      ...(!after ? (records ?? []).filter((record): record is Extract<ControlRecord, {kind: 'action'}> => record.kind === 'action' && record.status === 'pending').map(action => ({
        id: action.id, title: action.title, detail: `Proposal · ${projects.get(action.projectId) ?? 'Project'}`, open: review('workspaceAction', action.id),
      })) : []),
    ];
  }, [section, tasks.data, records, control.data?.decisions, owner, after, runtime.permissions, runtime.userInputs]);
  const error = control.query.error ?? tasks.error;
  const loading = !control.data && control.query.isFetching || !tasks.data && tasks.isFetching;
  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <FlatList data={rows} keyExtractor={row => row.id}
      contentContainerStyle={[styles.content, { paddingTop: header + 12 }]}
      refreshing={control.query.isFetching && Boolean(control.data)}
      onRefresh={() => { void control.query.refetch(); void tasks.refetch(); }}
      ListHeaderComponent={<View style={styles.header}>
        <ConnectionBanner />
        <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.tabs}>
          {sections.map(item => <Pressable key={item} accessibilityRole="tab" accessibilityState={{ selected: section === item }}
            onPress={() => { setSection(item); setAfter(null); setPrevious([]); }}
            style={[styles.tab, { backgroundColor: section === item ? theme.accentSoft : theme.surfaceMuted }]}>
            <Text style={[styles.tabLabel, { color: section === item ? theme.accent : theme.textSecondary }]}>{item}</Text>
          </Pressable>)}
        </ScrollView>
        <View style={styles.actions}>
          {section === 'Tasks' && <WorkbenchButton label="New task" primary onPress={() => router.push('/new-task')} />}
          {section === 'Projects' && <WorkbenchButton label="Open folder" primary onPress={() => setProjectPicker(true)} />}
          {section === 'Teams' && <><WorkbenchButton label="New team" primary onPress={() => router.push('/team-editor')} /><WorkbenchButton label="New agent" onPress={() => router.push('/agent-editor')} /></>}
        </View>
        {section === 'Tasks' && <WorkbenchCard><WorkbenchRow label="Workflows & routines" icon={{ android: 'account_tree', web: 'account_tree' }} onPress={() => router.push('/automations')} /></WorkbenchCard>}
        {section === 'Teams' && <WorkbenchCard><WorkbenchRow label="Apps" detail="Connect the accounts your agents use" icon={{ android: 'extension', web: 'extension' }} onPress={() => router.push('/apps')} /></WorkbenchCard>}
        {error && <WorkbenchState title="Could not refresh" detail={error instanceof Error ? error.message : 'Check your connection and retry.'}><WorkbenchButton label="Retry" onPress={() => { void control.query.refetch(); void tasks.refetch(); }} /></WorkbenchState>}
      </View>}
      ListEmptyComponent={control.daemon.phase !== 'connected' ? <WorkbenchState title="Connect to your desktop" detail="Open Settings to reconnect or choose another paired desktop."><WorkbenchButton label="Open Settings" onPress={() => router.push('/settings')} /></WorkbenchState> : loading ? <ActivityIndicator accessibilityLabel={`Loading ${section.toLowerCase()}`} color={theme.accent} style={styles.loading} /> : <WorkbenchState
        title={section === 'Decisions' ? 'All caught up' : `No ${section.toLowerCase()} yet`}
        detail={section === 'Decisions' ? 'Approvals and proposals that need your review appear here.' : section === 'Teams' ? 'Create an agent, then organize agents into a team.' : section === 'Projects' ? 'Open a folder on your paired desktop.' : 'Start a task to work with an agent.'} />}
      renderItem={({ item }) => <View style={styles.item}><WorkbenchCard>
        <WorkbenchRow label={item.title} detail={item.detail} icon={{ android: section === 'Projects' ? 'folder' : section === 'Teams' ? 'groups' : section === 'Decisions' ? 'rule' : 'chat_bubble_outline', web: 'work_outline' }} onPress={item.open} />
        {item.members?.map((member, index) => <Text key={index} style={[styles.member, { color: theme.textSecondary }]}>{member}</Text>)}
      </WorkbenchCard></View>}
      ListFooterComponent={section === 'Decisions' ? <View style={styles.actions}>
        {previous.length > 0 && <WorkbenchButton label="Previous" onPress={() => { setAfter(previous.at(-1) ?? null); setPrevious(previous.slice(0, -1)); }} />}
        {control.data?.next && <WorkbenchButton label="Next" onPress={() => { setPrevious([...previous, after]); setAfter(control.data!.next); }} />}
      </View> : null} />
    <RemoteProjectPicker visible={projectPicker} onDismiss={() => setProjectPicker(false)} onSelect={() => { void tasks.refetch(); }} />
  </View>;
}
const styles = StyleSheet.create({
  screen: { flex: 1 }, content: { width: '100%', maxWidth: 1000, alignSelf: 'center', padding: 16, paddingBottom: 24 }, header: { gap: 14, marginBottom: 16 },
  tabs: { gap: 8 }, tab: { minHeight: 48, paddingHorizontal: 16, paddingVertical: 12, justifyContent: 'center', borderRadius: 12 }, tabLabel: { fontSize: 14, fontWeight: '600' },
  actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8 }, item: { marginBottom: 10 }, member: { fontSize: 14, lineHeight: 20, paddingLeft: 14 }, loading: { padding: 32 },
});
