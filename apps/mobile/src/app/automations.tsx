import AsyncStorage from '@react-native-async-storage/async-storage';
import type { AutomationRun, Routine, Workflow } from '@waku/client';
import * as Crypto from 'expo-crypto';
import { router } from 'expo-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import { ActivityIndicator, FlatList, Pressable, RefreshControl, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AutomationButton, AutomationRefresh } from '@/components/automation-controls';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { automationStatus, routineTime, scheduleLabel, workflowTransition, startSavedAutomation } from '@/lib/automation-api';

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
      const response = await client.request({ type: 'setRoutinePaused', id: routine.id, expectedRevision: data.snapshot.revision, paused: !routine.paused });
      if (response.type !== 'workspaceControl') throw new Error('Unexpected schedule response. Refresh before retrying.');
    });
  }
  function changeProject(id: string | null) { setProject(id); setAfter(null); setPrevious([]); setExpanded(null); }

  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <FlatList data={rows} keyExtractor={(row) => row.id} contentContainerStyle={{ paddingTop: headerInset + 8, paddingBottom: insets.bottom + 24 }}
      refreshControl={<RefreshControl refreshing={query.isFetching} onRefresh={() => void query.refetch()} tintColor={theme.textSecondary} />}
      ListHeaderComponent={<View style={styles.header}>
        <ConnectionBanner />
        <AutomationRefresh refreshing={query.isFetching || daemon.phase !== 'connected'} onRefresh={() => void query.refetch()} />
        <View style={styles.actions}>{(['Workflows', 'Routines', 'Runs'] as const).map((value) => <Pressable key={value} accessibilityRole="tab" accessibilityState={{ selected: tab === value }}
          onPress={() => { setTab(value); setExpanded(null); }} style={[styles.tab, { backgroundColor: tab === value ? theme.overlayStrong : 'transparent' }]}>
          <Text style={[styles.body, { color: theme.text, fontWeight: tab === value ? '600' : '400' }]}>{value}</Text>
        </Pressable>)}</View>
        <FlatList horizontal data={[{ id: null, name: 'All projects' }, ...(taskState.data?.projects ?? []).map((p) => ({ id: p.id, name: p.name }))]}
          keyExtractor={(p) => p.id ?? 'all'} showsHorizontalScrollIndicator={false} contentContainerStyle={styles.projects}
          renderItem={({ item }) => <Pressable accessibilityRole="button" accessibilityState={{ selected: item.id === project }} onPress={() => changeProject(item.id)}
            style={[styles.tab, { backgroundColor: project === item.id ? theme.overlayStrong : 'transparent' }]}><Text style={[styles.body, { color: theme.text }]}>{item.name}</Text></Pressable>} />
        {!fresh && data && <Text style={[styles.body, { color: theme.textSecondary }]}>Saved status · refresh to use controls</Text>}
        {(message || query.error) && <Text accessibilityRole="alert" style={[styles.body, { color: theme.danger }]}>{message ?? query.error?.message}</Text>}
        {busy && <ActivityIndicator color={theme.accent} accessibilityLabel="Updating automation" />}
      </View>}
      ListEmptyComponent={<Text style={[styles.empty, { color: theme.textSecondary }]}>{query.isPending && query.isFetching ? 'Loading…' : tab === 'Runs' ? 'No runs on this page.' : `Create a ${tab === 'Workflows' ? 'workflow' : 'routine'} on desktop.`}</Text>}
      renderItem={({ item }) => {
        if (!('kind' in item)) return <Pressable accessibilityRole="button" accessibilityLabel={`Open ${item.name}`} onPress={() => router.push({ pathname: '/automation/[id]', params: { id: item.id } })}
          style={({ pressed }) => [styles.row, { borderBottomColor: theme.border, backgroundColor: pressed ? theme.overlay : 'transparent' }]}>
          <Text style={[styles.title, { color: theme.text }]}>{item.name}</Text>
          <Text style={[styles.body, { color: theme.textSecondary }]}>{names.get(item.projectId) ?? 'Project'} · {automationStatus(item.status)}</Text>
          <Text style={[styles.body, { color: theme.textSecondary }]}>{item.stages.filter((s) => s.completed).length} of {item.stages.length} stages</Text>
          {item.reason && <Text style={[styles.body, { color: theme.text }]}>{item.reason}</Text>}
        </Pressable>;
        return <View style={[styles.row, { borderBottomColor: theme.border }]}>
          <Pressable accessibilityRole="button" accessibilityLabel={`${expanded === item.id ? 'Hide' : 'Show'} ${item.name} instructions`} accessibilityState={{ expanded: expanded === item.id }} onPress={() => setExpanded(expanded === item.id ? null : item.id)} style={styles.definitionTitle}>
            <Text style={[styles.title, { color: theme.text }]}>{item.name}</Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>{names.get(item.projectId) ?? 'Project'} · {item.kind === 'workflow' ? `${item.stages.length} stages` : `${item.paused ? 'Paused' : 'Scheduled'} · ${scheduleLabel(item.intervalMinutes, item.calendar)}${item.paused ? '' : ` · ${routineTime(item.nextRun)}`}`}</Text>
          </Pressable>
          {expanded === item.id && <View style={styles.instructions}>
            <Text selectable style={[styles.body, { color: theme.text }]}>{item.kind === 'workflow' ? item.prompt : item.instructions}</Text>
            {item.kind === 'workflow' && item.stages.map((s, index) => <View key={index} style={styles.stage}>
              <Text style={[styles.body, { color: theme.text, fontWeight: '600' }]}>{index + 1}. {s.title}</Text>
              <Text style={[styles.body, { color: theme.textSecondary }]}>{s.teamId ? teamNames.get(s.teamId) ?? 'Team' : 'Choose a team on desktop'}{s.requiresDecision ? ' · User decision' : ''}</Text>
              <Text selectable style={[styles.body, { color: theme.text }]}>{s.instructions}</Text>
              {!!s.requiredFormats?.length && <Text style={[styles.body, { color: theme.textSecondary }]}>Required documents: {s.requiredFormats.join(', ').toUpperCase()}</Text>}
            </View>)}
          </View>}
          {item.kind === 'routine' && item.lastError && <Text style={[styles.body, { color: theme.danger }]}>{item.lastError}</Text>}
          <View style={styles.actions}>
            <AutomationButton label="Run now" onPress={() => run(item)} disabled={!fresh || busy !== null} />
            {item.kind === 'routine' && <AutomationButton label={item.paused ? 'Enable schedule' : 'Pause'} onPress={() => pause(item)} disabled={!fresh || busy !== null} />}
          </View>
        </View>;
      }}
      ListFooterComponent={tab === 'Runs' ? <View style={[styles.actions, styles.header]}>
        {previous.length > 0 && <AutomationButton label="Previous runs" disabled={!fresh || busy !== null} onPress={() => { const older = [...previous]; setAfter(older.pop() ?? null); setPrevious(older); }} />}
        {data?.history.nextCursor && <AutomationButton label="Next runs" disabled={!fresh || busy !== null} onPress={() => { setPrevious([...previous, after]); setAfter(data.history.nextCursor); }} />}
      </View> : null} />
  </View>;
}
const styles = StyleSheet.create({
  screen: { flex: 1 }, header: { paddingHorizontal: 20, gap: 12, paddingBottom: 16 },
  actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, alignItems: 'center' },
  projects: { gap: 4 }, tab: { paddingHorizontal: 14, minHeight: 44, justifyContent: 'center', borderRadius: 8 },
  row: { paddingHorizontal: 20, paddingVertical: 18, gap: 8, borderBottomWidth: StyleSheet.hairlineWidth },
  definitionTitle: { minHeight: 44, gap: 6, justifyContent: 'center' },
  title: { fontSize: 17, fontWeight: '600', lineHeight: 24 }, body: { fontSize: 14, lineHeight: 21 },
  instructions: { gap: 14, paddingVertical: 8 }, stage: { gap: 6 }, empty: { padding: 24, fontSize: 15, lineHeight: 22 },
});
