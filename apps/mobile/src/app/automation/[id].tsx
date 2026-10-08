import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { ActivityIndicator, FlatList, RefreshControl, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AutomationButton, AutomationRefresh } from '@/components/automation-controls';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTheme } from '@/hooks/use-theme';
import { automationStatus, routineTime, triggerLabel, workflowTarget, workflowTransition, runResponse } from '@/lib/automation-api';

export default function AutomationScreen() {
  const { id: value } = useLocalSearchParams<{ id: string | string[] }>();
  const id = Array.isArray(value) ? value[0] : value;
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const headerInset = useScreenHeaderInset();
  const { data: run, query, daemon, fresh } = useAutomationQuery(['run', id], async (client) => {
    if (!id) throw new Error('No automation selected. Open it from Workflows & routines.');
    return runResponse(client, { type: 'getAutomationRun', id });
  });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const locked = useRef(false);
  const scope = `${daemon.activeProfile?.id}:${daemon.activeProfile?.address}:${id}`;
  const scopeRef = useRef(scope); scopeRef.current = scope;
  useEffect(() => setMessage(null), [scope]);

  async function act(approved?: boolean) {
    const client = daemon.client;
    if (!client || !fresh || !run || locked.current) return;
    locked.current = true; setBusy(true); setMessage(null);
    const startedScope = scope;
    try {
      const result = await runResponse(client, approved === undefined
        ? { type: 'stopAutomation', id: run.id, expectedRevision: run.revision }
        : { type: 'resolveAutomationGate', id: run.id, expectedRevision: run.revision, stage: run.currentStage, approved });
      if (result.id !== run.id) throw new Error('Another run was returned. Refresh before retrying.');
      if (scopeRef.current === startedScope) await query.refetch();
    } catch (cause) {
      if (scopeRef.current === startedScope) {
        setMessage(cause instanceof Error ? cause.message : String(cause));
        // An acknowledgement can be lost after the exact decision commits.
        // Inspect owner state; never create a replacement stage on the phone.
        await query.refetch();
      }
    } finally { locked.current = false; setBusy(false); }
  }
  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <FlatList data={run?.stages ?? []} keyExtractor={(stage) => stage.teamRunId}
      contentContainerStyle={{ paddingTop: headerInset + 8, paddingBottom: insets.bottom + 24 }}
      refreshControl={<RefreshControl refreshing={query.isFetching} onRefresh={() => void query.refetch()} tintColor={theme.textSecondary} />}
      ListHeaderComponent={<View style={styles.header}>
        <ConnectionBanner />
        <AutomationRefresh refreshing={query.isFetching || daemon.phase !== 'connected'} onRefresh={() => void query.refetch()} />
        {run && <>
          <Text style={[styles.title, { color: theme.text }]}>{run.name}</Text>
          <Text style={[styles.body, { color: theme.textSecondary }]}>{automationStatus(run.status)} · {run.stages.filter((s) => s.completed).length} of {run.stages.length} stages</Text>
          <Text style={[styles.body, { color: theme.textSecondary }]}>Deadline {routineTime(run.deadline)}</Text>
          {run.trigger && <Text style={[styles.body, { color: theme.textSecondary }]}>{triggerLabel(run.trigger)}</Text>}
          {run.reason && <Text style={[styles.body, { color: theme.text }]}>{run.reason}</Text>}
        </>}
        {!fresh && run && <Text style={[styles.body, { color: theme.textSecondary }]}>Saved status · refresh to use controls</Text>}
        {(message || query.error) && <Text accessibilityRole="alert" style={[styles.body, { color: theme.danger }]}>{message ?? query.error?.message}</Text>}
        {busy && <ActivityIndicator color={theme.accent} accessibilityLabel="Updating run" />}
        {run && (run.status === 'running' || run.status === 'blocked' || run.status === 'waitingDecision') &&
          <View style={styles.actions}><AutomationButton label="Stop run" destructive disabled={!fresh || busy} onPress={() => void act()} /></View>}
      </View>}
      ListEmptyComponent={!query.error ? <Text style={[styles.empty, { color: theme.textSecondary }]}>{query.isFetching ? 'Loading…' : 'Connect and refresh to view this run.'}</Text> : null}
      renderItem={({ item: stage, index }) => <View style={[styles.stage, { borderBottomColor: theme.border }]}>
        <Text style={[styles.stageTitle, { color: theme.text }]}>{index + 1}. {stage.title}</Text>
        <Text style={[styles.body, { color: theme.textSecondary }]}>{stage.skipped ? 'Skipped by workflow route' : stage.completed ? 'Completed' : stage.started ? 'Started · open task for progress' : run?.status === 'waitingDecision' && index === run.currentStage ? 'Awaiting your decision' : 'Not started'}</Text>
        <Text selectable style={[styles.body, { color: theme.text }]}>{stage.instructions}</Text>
        <Text style={[styles.body, { color: theme.textSecondary }]}>{workflowTransition(stage.transition)}</Text>
        {stage.route && <Text style={[styles.body, { color: theme.textSecondary }]}>Chosen: {workflowTarget(stage.route.target)}{stage.route.matched == null ? '' : stage.route.matched ? ' (matched)' : ' (did not match)'}</Text>}
        {stage.route?.artifactId && <Text selectable style={[styles.body, { color: theme.textSecondary }]}>Document {stage.route.artifactId} · SHA-256 {stage.route.sha256}</Text>}
        {!!stage.requiredFormats?.length && <Text style={[styles.body, { color: theme.textSecondary }]}>Required documents: {stage.requiredFormats.join(', ').toUpperCase()}</Text>}
        <View style={styles.actions}>
          {stage.started && <AutomationButton label="Open task" onPress={() => router.push({ pathname: '/session/[id]', params: { id: stage.taskId } })} />}
          {run?.status === 'waitingDecision' && index === run.currentStage && <>
            <AutomationButton label="Approve stage" disabled={!fresh || busy} onPress={() => void act(true)} />
            <AutomationButton label="Decline stage" disabled={!fresh || busy} onPress={() => void act(false)} />
          </>}
        </View>
      </View>} />
  </View>;
}
const styles = StyleSheet.create({
  screen: { flex: 1 }, header: { paddingHorizontal: 20, gap: 12, paddingBottom: 20 },
  title: { fontSize: 22, fontWeight: '600', lineHeight: 30 }, body: { fontSize: 14, lineHeight: 21 },
  stageTitle: { fontSize: 17, fontWeight: '600', lineHeight: 24 },
  stage: { paddingHorizontal: 20, paddingVertical: 20, gap: 10, borderBottomWidth: StyleSheet.hairlineWidth },
  actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8 }, empty: { padding: 24, fontSize: 15, lineHeight: 22 },
});
