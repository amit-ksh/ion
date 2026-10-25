import AsyncStorage from '@react-native-async-storage/async-storage';
import type { AutomationRun, Routine, Workflow } from '@waku/client';
import * as Crypto from 'expo-crypto';
import { router } from 'expo-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import { ActivityIndicator, FlatList, RefreshControl, StyleSheet, Switch, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AppSymbol } from '@/components/app-symbol';
import { AutomationButton, AutomationRefresh } from '@/components/automation-controls';
import { Chip } from '@/components/chip';
import { EntityRow, rowPosition } from '@/components/entity-row';
import { OverflowMenu } from '@/components/overflow-menu';
import { RunStatusIcon, StageProgress, runStatus } from '@/components/run-row';
import { SegmentedTabs } from '@/components/segmented-tabs';
import { StatusPill } from '@/components/ticket-card';
import { WorkbenchButton, WorkbenchState } from '@/components/workbench';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { automationStatus, scheduleLabel, startSavedAutomation, requestMobileAutomation } from '@/lib/automation-api';
import { ticketNumber } from '@/lib/ticket';
import { localTimeLabel, relativeTimeLabel } from '@/lib/time-labels';

type Tab = 'Workflows' | 'Routines' | 'Runs';
type Definition = (Workflow & { kind: 'workflow' }) | (Routine & { kind: 'routine' });
export default function AutomationsScreen() {
  const theme = useTheme();
  const headerInset = useScreenHeaderInset();
  const insets = useSafeAreaInsets();
  const taskState = useTaskState();
  const [tab, setTab] = useState<Tab>('Workflows');
  const [project, setProject] = useState<string | null>(null);
  const [after, setAfter] = useState<string | null>(null);
  const [previous, setPrevious] = useState<(string | null)[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const locked = useRef(false);
  const state = useAutomationQuery(['list', project, after], async (client) => {
    const [definitions, history] = await Promise.all([
      client.request({ type: 'loadWorkspaceControl' }),
      client.request({ type: 'listAutomationRuns', pendingOnly: false, projectId: project, definitionId: null, after, limit: 32 }),
    ]);
    if (definitions.type !== 'workspaceControl' || history.type !== 'automationRuns') throw new Error('Unexpected automation collection. Refresh to retry.');
    return { snapshot: definitions.snapshot, history: history.page };
  });
  const { data, daemon, fresh, query } = state;
  const scope = `${daemon.activeProfile?.id}:${daemon.activeProfile?.address}`;
  const scopeRef = useRef(scope); scopeRef.current = scope;
  const mounted = useRef(true);
  const visible = useRef(state.visible); visible.current = state.visible;
  useEffect(() => () => { mounted.current = false; }, []);
  useEffect(() => { setProject(null); setAfter(null); setPrevious([]); setExpanded(null); setMessage(null); }, [scope]);
  const definitions = useMemo(() => data?.snapshot.records.filter((r): r is Definition =>
    (r.kind === 'workflow' || r.kind === 'routine') && (!project || r.projectId === project)) ?? [], [data?.snapshot, project]);
  const names = useMemo(() => new Map(taskState.data?.projects.map((p) => [p.id, p.name]) ?? []), [taskState.data]);
  const teamNames = useMemo(() => new Map(data?.snapshot.records.filter((r) => r.kind === 'team').map((r) => [r.id, r.name]) ?? []), [data?.snapshot]);
  const rows: (Definition | AutomationRun)[] = tab === 'Runs' ? data?.history.runs ?? [] : definitions.filter((r) => r.kind === (tab === 'Workflows' ? 'workflow' : 'routine'));

  async function action(id: string, operation: () => Promise<void>) {
    if (!fresh || !daemon.client || locked.current) return;
    const startedScope = scope;
    locked.current = true; setBusy(id); setMessage(null);
    try { await operation(); if (scopeRef.current === startedScope) await query.refetch(); }
    catch (cause) { if (scopeRef.current === startedScope) setMessage(cause instanceof Error ? cause.message : String(cause)); }
    finally { locked.current = false; setBusy(null); }
  }
  function run(definition: Definition) {
    const client = daemon.client;
    if (!client || !data) return;
    void action(definition.id, async () => {
      const result = await startSavedAutomation(client, AsyncStorage, scope, definition.id, data.snapshot.revision, Crypto.randomUUID);
      if (mounted.current && visible.current && scopeRef.current === scope) router.push({ pathname: '/automation/[id]', params: { id: result.id } });
    });
  }
  function pause(routine: Routine) {
    const client = daemon.client;
    if (!client || !data) return;
    void action(routine.id, async () => {
      const response = await requestMobileAutomation(client, { type: 'setRoutinePaused', id: routine.id, expectedRevision: data.snapshot.revision, paused: !routine.paused });
      if (response.type !== 'workspaceControl') throw new Error('Unexpected schedule response. Refresh before retrying.');
    });
  }
  function changeProject(id: string | null) { setProject(id); setAfter(null); setPrevious([]); setExpanded(null); }

  const runs = data?.history.runs ?? [];
  const lastRuns = new Map<string, AutomationRun>();
  for (const item of runs) {
    const key = item.routineId ?? item.workflowId;
    if (key && !lastRuns.has(key)) lastRuns.set(key, item);
  }
  const disabled = !fresh || busy !== null;
  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <FlatList data={rows} keyExtractor={(row) => row.id} contentContainerStyle={[styles.content, { paddingTop: headerInset + 8, paddingBottom: insets.bottom + 24 }]}
      refreshControl={<RefreshControl refreshing={query.isFetching} onRefresh={() => void query.refetch()} tintColor={theme.textSecondary} />}
      ListHeaderComponent={<View style={styles.header}>
        <ConnectionBanner />
        <SegmentedTabs tabs={TABS} value={tab} onChange={(value) => { setTab(value); setExpanded(null); }} />
        <FlatList horizontal data={[{ id: null, name: 'All projects' }, ...(taskState.data?.projects ?? []).map((p) => ({ id: p.id, name: p.name }))]}
          keyExtractor={(p) => p.id ?? 'all'} showsHorizontalScrollIndicator={false} style={styles.projectStrip} contentContainerStyle={styles.projects}
          renderItem={({ item }) => <Chip label={item.name} chevron={false} active={item.id === project}
            icon={item.id ? { android: 'folder', web: 'folder' } : { android: 'select_all', web: 'select_all' }}
            accessibilityLabel={item.id === project ? `${item.name}, selected` : `Show ${item.name}`} onPress={() => changeProject(item.id)} />} />
        <View style={styles.toolbar}>
          {tab !== 'Runs' && <WorkbenchButton label={tab === 'Workflows' ? 'New workflow' : 'New routine'} primary icon={{ android: 'add', web: 'add' }} disabled={disabled}
            onPress={() => router.push({ pathname: '/automation-editor', params: { kind: tab === 'Workflows' ? 'workflow' : 'routine', ...(project ? { projectId: project } : {}) } })} />}
          <View style={styles.grow} />
          {busy && <ActivityIndicator color={theme.accent} accessibilityLabel="Updating automation" />}
          <AutomationRefresh refreshing={query.isFetching || daemon.phase !== 'connected'} onRefresh={() => void query.refetch()} />
        </View>
        {!fresh && data && <Text style={[styles.body, { color: theme.textSecondary }]}>Saved status · refresh to use controls</Text>}
        {(message || query.error) && <Text accessibilityRole="alert" style={[styles.body, { color: theme.danger }]}>{message ?? query.error?.message}</Text>}
      </View>}
      ListEmptyComponent={query.isPending && query.isFetching ? <ActivityIndicator accessibilityLabel="Loading automations" color={theme.accent} style={styles.loading} />
        : <WorkbenchState icon={tab === 'Runs' ? { android: 'history', web: 'history' } : tab === 'Workflows' ? { android: 'account_tree', web: 'account_tree' } : { android: 'schedule', web: 'schedule' }}
          title={tab === 'Runs' ? 'No runs on this page' : `No ${tab === 'Workflows' ? 'workflows' : 'routines'} yet`}
          detail={tab === 'Runs' ? 'Runs appear here when a workflow or routine starts.' : tab === 'Workflows' ? 'Chain team stages into a repeatable workflow.' : 'Schedule an agent to run on its own.'} />}
      renderItem={({ item, index }) => {
        const position = rowPosition(index, rows.length);
        if (!('kind' in item)) return <EntityRow position={position}
          leading={<RunStatusIcon status={item.status} />}
          identifier={ticketNumber(item.id)} title={item.name}
          subtitle={`${names.get(item.projectId) ?? 'Project'} · ${relativeTimeLabel(item.createdAt)}`}
          accessibilityLabel={`Open ${item.name}, ${automationStatus(item.status)}`}
          onPress={() => router.push({ pathname: '/automation/[id]', params: { id: item.id } })}
          footer={<>
            <View style={styles.runStatus}>
              <StatusPill {...runStatus(item.status)} />
              <View style={styles.runProgress}><StageProgress run={item} /></View>
            </View>
            {item.reason && <Text numberOfLines={2} style={[styles.small, { color: theme.textSecondary }]}>{item.reason}</Text>}
          </>} />;
        const routine = item.kind === 'routine' ? item : null;
        const last = lastRuns.get(item.id);
        const schedule = routine
          ? `${scheduleLabel(routine.intervalMinutes, routine.calendar)} · ${routine.paused ? 'Paused' : `Next ${localTimeLabel(routine.nextRun)}`}`
          : `${item.kind === 'workflow' ? item.stages.length : 0} stages`;
        const lastRun = last ? `Last run ${automationStatus(last.status).toLowerCase()} ${relativeTimeLabel(last.createdAt)}` : 'No runs yet';
        const open = expanded === item.id;
        return <EntityRow position={position}
          leading={<View style={[styles.kindIcon, { backgroundColor: theme.surfaceMuted }]}>
            <AppSymbol name={routine ? { android: 'schedule', web: 'schedule' } : { android: 'account_tree', web: 'account_tree' }} size={16} tintColor={theme.textSecondary} />
          </View>}
          title={item.name}
          subtitle={`${names.get(item.projectId) ?? 'Project'} · ${schedule}`}
          subtitleLines={2}
          meta={lastRun}
          accessibilityLabel={`${open ? 'Hide' : 'Show'} ${item.name} instructions`}
          accessibilityHint={`${schedule}. ${lastRun}`}
          onPress={() => setExpanded(open ? null : item.id)}
          footer={<>
            {routine?.lastError && <Text style={[styles.small, { color: theme.danger }]}>{routine.lastError}</Text>}
            {open && <View style={[styles.instructions, { borderColor: theme.border }]}>
              <Text selectable style={[styles.body, { color: theme.text }]}>{item.kind === 'workflow' ? item.prompt : item.instructions}</Text>
              {item.kind === 'workflow' && item.stages.map((s, stage) => <View key={stage} style={styles.stage}>
                <Text style={[styles.body, { color: theme.text, fontWeight: '600' }]}>{stage + 1}. {s.title}</Text>
                <Text style={[styles.small, { color: theme.textSecondary }]}>{s.teamId ? teamNames.get(s.teamId) ?? 'Team' : 'Choose a team'}{s.requiresDecision ? ' · User decision' : ''}</Text>
                <Text selectable style={[styles.body, { color: theme.text }]}>{s.instructions}</Text>
                <Text style={[styles.small, { color: theme.textSecondary }]}>{s.retry ? `Retry before effects: up to ${s.retry.maxAttempts - 1} · ${s.retry.delaySeconds}s delay` : 'Retries: off'}</Text>
                {s.inheritWorkspace && <Text style={[styles.small, { color: theme.textSecondary }]}>Inherit code: on</Text>}
                {!!s.requiredFormats?.length && <Text style={[styles.small, { color: theme.textSecondary }]}>Required documents: {s.requiredFormats.join(', ').toUpperCase()}</Text>}
              </View>)}
            </View>}
            <View style={styles.controls}>
              <WorkbenchButton label={busy === item.id ? 'Starting…' : 'Run now'} icon={{ android: 'play_arrow', web: 'play_arrow' }} disabled={disabled} onPress={() => run(item)} />
              {routine && <View style={styles.toggle}>
                <Switch accessibilityLabel={routine.paused ? `Enable ${routine.name}` : `Pause ${routine.name}`} value={!routine.paused} disabled={disabled}
                  onValueChange={() => pause(routine)} trackColor={{ false: theme.overlayStrong, true: theme.primary }}
                  thumbColor={routine.paused ? theme.textTertiary : theme.onPrimary} />
                <Text style={[styles.small, styles.toggleLabel, { color: theme.textSecondary }]}>{routine.paused ? 'Off' : 'On'}</Text>
              </View>}
              <View style={styles.grow} />
              <OverflowMenu label={`More actions for ${item.name}`} disabled={disabled}
                actions={[{ id: 'edit', title: 'Edit', image: 'pencil' }, { id: 'instructions', title: open ? 'Hide instructions' : 'Show instructions', image: 'doc.text' },
                  ...(routine ? [{ id: 'toggle', title: routine.paused ? 'Enable schedule' : 'Pause' }] : [])]}
                onAction={(id) => {
                  if (id === 'edit') router.push({ pathname: '/automation-editor', params: { id: item.id, kind: item.kind } });
                  else if (id === 'instructions') setExpanded(open ? null : item.id);
                  else if (id === 'toggle' && routine) pause(routine);
                }} />
            </View>
          </>} />;
      }}
      ListFooterComponent={tab === 'Runs' ? <View style={styles.toolbar}>
        {previous.length > 0 && <AutomationButton label="Previous runs" disabled={disabled} onPress={() => { const older = [...previous]; setAfter(older.pop() ?? null); setPrevious(older); }} />}
        {data?.history.nextCursor && <AutomationButton label="Next runs" disabled={disabled} onPress={() => { setPrevious([...previous, after]); setAfter(data.history.nextCursor); }} />}
      </View> : null} />
  </View>;
}
const TABS = ['Workflows', 'Routines', 'Runs'] as const;
const styles = StyleSheet.create({
  screen: { flex: 1 }, content: { width: '100%', maxWidth: 1000, alignSelf: 'center', paddingHorizontal: 16 },
  header: { gap: 12, paddingBottom: 16 },
  toolbar: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, alignItems: 'center' },
  projectStrip: { marginHorizontal: -16 }, projects: { gap: 6, paddingHorizontal: 16 }, grow: { flexGrow: 1 }, loading: { padding: 32 },
  kindIcon: { alignItems: 'center', borderRadius: 16, height: 32, justifyContent: 'center', width: 32 },
  runStatus: { alignItems: 'center', flexDirection: 'row', gap: 12 },
  runProgress: { flex: 1, minWidth: 0 },
  controls: { alignItems: 'center', flexDirection: 'row', flexWrap: 'wrap', gap: 8 },
  toggle: { alignItems: 'center', flexDirection: 'row', gap: 4, minHeight: 48 },
  toggleLabel: { minWidth: 24 },
  body: { fontSize: 14, lineHeight: 21 }, small: { fontSize: 12.5, lineHeight: 18 },
  instructions: { borderTopWidth: StyleSheet.hairlineWidth, gap: 14, paddingTop: 12 }, stage: { gap: 6 },
});
