import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { ActivityIndicator, FlatList, RefreshControl, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AppSymbol } from '@/components/app-symbol';
import { AutomationRefresh } from '@/components/automation-controls';
import { EntityRow, rowPosition } from '@/components/entity-row';
import { RunStatusIcon, StageProgress, runStatus } from '@/components/run-row';
import { StatusPill } from '@/components/ticket-card';
import { WorkbenchButton, WorkbenchState } from '@/components/workbench';
import { MonoFont } from '@/constants/theme';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTheme } from '@/hooks/use-theme';
import { triggerLabel, workflowTarget, workflowTransition, runResponse } from '@/lib/automation-api';
import { ticketNumber } from '@/lib/ticket';
import { localTimeLabel, relativeTimeLabel } from '@/lib/time-labels';

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
  const done = run?.stages.filter((s) => s.completed).length ?? 0;
  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <FlatList data={run?.stages ?? []} keyExtractor={(stage) => stage.teamRunId}
      contentContainerStyle={[styles.content, { paddingTop: headerInset + 8, paddingBottom: insets.bottom + 24 }]}
      refreshControl={<RefreshControl refreshing={query.isFetching} onRefresh={() => void query.refetch()} tintColor={theme.textSecondary} />}
      ListHeaderComponent={<View style={styles.header}>
        <ConnectionBanner />
        {!run && <AutomationRefresh refreshing={query.isFetching || daemon.phase !== 'connected'} onRefresh={() => void query.refetch()} />}
        {run && <View style={[styles.summary, { backgroundColor: theme.ticket, borderColor: theme.ticketBorder, boxShadow: theme.elevationRest }]}>
          <View style={styles.summaryHead}>
            <RunStatusIcon status={run.status} />
            <View style={styles.summaryCopy}>
              <Text style={[styles.mono, { color: theme.textTertiary }]}>{ticketNumber(run.id)} · {relativeTimeLabel(run.createdAt)}</Text>
              <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>{run.name}</Text>
            </View>
            <StatusPill {...runStatus(run.status)} />
          </View>
          <StageProgress run={run} />
          <Text style={[styles.body, { color: theme.textSecondary }]}>{done} of {run.stages.length} {run.stages.length === 1 ? 'stage' : 'stages'} · Deadline {localTimeLabel(run.deadline)}</Text>
          {run.trigger && <Text style={[styles.body, { color: theme.textSecondary }]}>{triggerLabel(run.trigger)}</Text>}
          {run.reason && <Text style={[styles.body, { color: theme.text }]}>{run.reason}</Text>}
          <View style={styles.actions}>
            {(run.status === 'running' || run.status === 'blocked' || run.status === 'waitingDecision') &&
              <WorkbenchButton label="Stop run" destructive icon={{ android: 'stop', web: 'stop' }} disabled={!fresh || busy} onPress={() => void act()} />}
            <View style={styles.grow} />
            <AutomationRefresh refreshing={query.isFetching || daemon.phase !== 'connected'} onRefresh={() => void query.refetch()} />
          </View>
        </View>}
        {!fresh && run && <Text style={[styles.body, { color: theme.textSecondary }]}>Saved status · refresh to use controls</Text>}
        {(message || query.error) && <Text accessibilityRole="alert" style={[styles.body, { color: theme.danger }]}>{message ?? query.error?.message}</Text>}
        {busy && <ActivityIndicator color={theme.accent} accessibilityLabel="Updating run" />}
        {run && run.stages.length > 0 && <Text accessibilityRole="header" style={[styles.section, { color: theme.text }]}>Stages</Text>}
      </View>}
      ListEmptyComponent={!query.error ? (query.isFetching ? <ActivityIndicator accessibilityLabel="Loading run" color={theme.accent} style={styles.loading} />
        : <WorkbenchState icon={{ android: 'link_off', web: 'link_off' }} title="Run unavailable" detail="Connect and refresh to view this run." />) : null}
      renderItem={({ item: stage, index }) => {
        const deciding = run?.status === 'waitingDecision' && index === run.currentStage;
        const state = stage.skipped ? 'Skipped by workflow route' : stage.completed ? 'Completed' : deciding ? 'Awaiting your decision' : stage.started ? 'Started · open task for progress' : 'Not started';
        const icon = stage.completed ? { android: 'check', web: 'check' } as const : stage.skipped ? { android: 'skip_next', web: 'skip_next' } as const
          : deciding ? { android: 'error', web: 'error' } as const : stage.started ? { android: 'progress_activity', web: 'progress_activity' } as const : { android: 'radio_button_unchecked', web: 'radio_button_unchecked' } as const;
        const tint = stage.completed ? theme.success : deciding || stage.started ? theme.accent : theme.textTertiary;
        return <EntityRow position={rowPosition(index, run?.stages.length ?? 0)}
          leading={<View style={[styles.stageIcon, { backgroundColor: theme.surfaceMuted }]}><AppSymbol name={icon} size={16} tintColor={tint} /></View>}
          identifier={`${index + 1}`} title={stage.title} subtitle={state}
          footer={<>
            <Text selectable style={[styles.body, { color: theme.text }]}>{stage.instructions}</Text>
            <Text style={[styles.small, { color: theme.textSecondary }]}>{workflowTransition(stage.transition)}</Text>
            <Text style={[styles.small, { color: theme.textSecondary }]}>{stage.retry ? `Retry before effects: up to ${stage.retry.maxAttempts - 1} · ${stage.retry.delaySeconds}s delay` : 'Retries: off'}</Text>
            {stage.inheritWorkspace && <Text style={[styles.small, { color: theme.textSecondary }]}>{stage.workspaceFrom && run?.stages.some(s => s.teamRunId === stage.workspaceFrom) ? `Code from stage ${run.stages.findIndex(s => s.teamRunId === stage.workspaceFrom) + 1}` : 'Inherit code: on'}</Text>}
            {stage.route && <Text style={[styles.small, { color: theme.textSecondary }]}>Chosen: {workflowTarget(stage.route.target)}{stage.route.matched == null ? '' : stage.route.matched ? ' (matched)' : ' (did not match)'}</Text>}
            {stage.route?.artifactId && <Text selectable style={[styles.small, styles.mono, { color: theme.textSecondary }]}>Document {stage.route.artifactId} · SHA-256 {stage.route.sha256}</Text>}
            {!!stage.requiredFormats?.length && <Text style={[styles.small, { color: theme.textSecondary }]}>Required documents: {stage.requiredFormats.join(', ').toUpperCase()}</Text>}
            {(stage.started || deciding) && <View style={styles.actions}>
              {stage.started && <WorkbenchButton label="Open task" icon={{ android: 'open_in_new', web: 'open_in_new' }} onPress={() => router.push({ pathname: '/session/[id]', params: { id: stage.taskId } })} />}
              {deciding && <>
                <WorkbenchButton label="Approve on desktop" primary disabled={!fresh || busy} onPress={() => void act(true)} />
                <WorkbenchButton label="Decline stage" destructive disabled={!fresh || busy} onPress={() => void act(false)} />
              </>}
            </View>}
          </>} />;
      }} />
  </View>;
}
const styles = StyleSheet.create({
  screen: { flex: 1 }, content: { width: '100%', maxWidth: 1000, alignSelf: 'center', paddingHorizontal: 16 },
  header: { gap: 12, paddingBottom: 12 },
  summary: { borderRadius: 16, borderWidth: 1, gap: 12, padding: 16 },
  summaryHead: { alignItems: 'flex-start', flexDirection: 'row', gap: 12 },
  summaryCopy: { flex: 1, gap: 2, minWidth: 0 },
  title: { fontSize: 19, fontWeight: '600', lineHeight: 26 }, body: { fontSize: 14, lineHeight: 21 }, small: { fontSize: 12.5, lineHeight: 18 },
  mono: { fontFamily: MonoFont, fontSize: 12, fontVariant: ['tabular-nums'], lineHeight: 17 },
  section: { fontSize: 15, fontWeight: '600', lineHeight: 21, paddingHorizontal: 4, paddingTop: 8 },
  stageIcon: { alignItems: 'center', borderRadius: 16, height: 32, justifyContent: 'center', width: 32 },
  actions: { alignItems: 'center', flexDirection: 'row', flexWrap: 'wrap', gap: 8 }, grow: { flexGrow: 1 }, loading: { padding: 32 },
});
