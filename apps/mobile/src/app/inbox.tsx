import type { InboxAction, InboxEntry, InboxHistoryStatus, InboxHistoryView, InboxKind, NotificationPolicy } from '@waku/client';
import { router } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { ActivityIndicator, FlatList, Pressable, RefreshControl, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AutomationButton, AutomationRefresh } from '@/components/automation-controls';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { routineTime } from '@/lib/automation-api';

const VIEWS: [InboxHistoryView, string][] = [['inbox', 'Inbox'], ['unread', 'Unread'], ['acknowledged', 'Acknowledged'], ['archived', 'Archived'], ['all', 'All notifications']];
const KINDS: [InboxKind | null, string][] = [[null, 'All types'], ['proposal', 'Proposed tasks'], ['decision', 'Decisions'], ['approval', 'Approvals'], ['question', 'Questions'], ['issue', 'Issues'], ['completed', 'Completed work'], ['artifact', 'Documents'], ['update', 'Updates']];
const STATUSES: [InboxHistoryStatus, string][] = [['all', 'All statuses'], ['needsAction', 'Needs action'], ['resolved', 'Resolved']];
const POLICIES: [NotificationPolicy, string][] = [['all', 'All notifications'], ['issuesOnly', 'Issues only'], ['off', 'Off']];
export default function InboxScreen() {
  const theme = useTheme(); const headerInset = useScreenHeaderInset(); const insets = useSafeAreaInsets(); const tasks = useTaskState();
  const [project, setProject] = useState<string | null>(null); const [task, setTask] = useState<string | null>(null);
  const [view, setView] = useState<InboxHistoryView>('inbox'); const [kind, setKind] = useState<InboxKind | null>(null); const [status, setStatus] = useState<InboxHistoryStatus>('all');
  const [after, setAfter] = useState<number | null>(null); const [previous, setPrevious] = useState<(number | null)[]>([]);
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState<string | null>(null); const locked = useRef(false); const operationGeneration = useRef(0);
  const state = useAutomationQuery(['inbox', project, task, view, kind, status, after], async (client) => {
    const response = await client.request({ type: 'listInboxHistory', query: { projectId: project, taskId: task, kind, view, status, after, limit: 32 } });
    if (response.type !== 'inboxHistory') throw new Error('Unexpected Inbox history response. Refresh to retry.');
    return response.page;
  });
  const { data: page, query, fresh, daemon } = state;
  const scope = `${daemon.activeProfile?.id}:${daemon.activeProfile?.address}:${daemon.connections}`;
  const scopeRef = useRef(scope); scopeRef.current = scope;
  const mounted = useRef(true); const visible = useRef(state.visible); visible.current = state.visible;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => { setProject(null); setTask(null); setView('inbox'); setKind(null); setStatus('all'); resetPaging(); setMessage(null); operationGeneration.current += 1; locked.current = false; setBusy(false); }, [scope]);
  const projectNames = new Map(tasks.data?.projects.map((p) => [p.id, p.name]) ?? []);
  function resetPaging() { setAfter(null); setPrevious([]); }
  async function action(operation: () => Promise<void>) {
    if (!fresh || !daemon.client || !page || locked.current) return;
    const startedScope = scope; const ticket = ++operationGeneration.current; locked.current = true; setBusy(true); setMessage(null);
    try { await operation(); }
    catch (cause) { if (mounted.current && operationGeneration.current === ticket && scopeRef.current === startedScope) setMessage(cause instanceof Error ? cause.message : String(cause)); }
    finally {
      if (operationGeneration.current !== ticket) return;
      locked.current = false;
      if (mounted.current && scopeRef.current === startedScope) { setBusy(false); if (visible.current) await query.refetch(); }
    }
  }
  function update(entry: InboxEntry, actionKind: InboxAction) {
    const client = daemon.client; if (!client || !page) return;
    void action(async () => {
      const response = await client.request({ type: 'updateInboxItem', expectedRevision: page.revision, key: entry.state.key, action: actionKind });
      if (response.type !== 'inbox') throw new Error('The update was not confirmed. Refresh before retrying.');
    });
  }
  function policy(value: NotificationPolicy) {
    const client = daemon.client; if (!client || !page) return;
    void action(async () => {
      const response = await client.request({ type: 'setNotificationPolicy', expectedRevision: page.revision, policy: value });
      if (response.type !== 'inbox') throw new Error('The preference was not confirmed. Refresh before retrying.');
    });
  }
  function open(entry: InboxEntry) {
    if (!fresh || !visible.current) return;
    if (entry.source === 'teamDecision' || entry.source === 'workspaceAction') router.push({ pathname: '/inbox-source', params: { source: entry.source, id: entry.sourceId, run: entry.runId ?? '', owner: daemon.activeProfile!.id } });
    else if (entry.source === 'automation') router.push({ pathname: '/automation/[id]', params: { id: entry.sourceId } });
    else if (entry.taskId) router.push({ pathname: '/session/[id]', params: { id: entry.taskId } });
    else setMessage('This source is available in the desktop workspace.');
  }
  function choices<T extends string | null>(values: readonly [T, string][], selected: T, change: (value: T) => void, label: string) {
    return <FlatList horizontal accessibilityLabel={label} data={values} keyExtractor={([value]) => value ?? 'all'} showsHorizontalScrollIndicator={false} contentContainerStyle={styles.filters}
      renderItem={({ item: [value, name] }) => <Pressable accessibilityRole="button" accessibilityState={{ selected: selected === value }} onPress={() => { change(value); resetPaging(); }} style={[styles.filter, { backgroundColor: selected === value ? theme.overlayStrong : 'transparent' }]}>
        <Text style={[styles.body, { color: theme.text }]}>{name}</Text>
      </Pressable>} />;
  }
  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <FlatList data={page?.entries ?? []} keyExtractor={(entry) => entry.state.key}
      contentContainerStyle={{ paddingTop: headerInset + 8, paddingBottom: insets.bottom + 24 }}
      refreshControl={<RefreshControl refreshing={query.isFetching} onRefresh={() => void query.refetch()} tintColor={theme.textSecondary} />}
      ListHeaderComponent={<View style={styles.header}>
        <ConnectionBanner /><AutomationRefresh refreshing={query.isFetching || daemon.phase !== 'connected'} onRefresh={() => void query.refetch()} />
        {page && <Text style={[styles.body, { color: theme.textSecondary }]}>{page.matchingCount} notifications · {page.unreadCount} unread across projects</Text>}
        {page?.taskHistorySyncing && <Text style={[styles.body, { color: theme.textSecondary }]}>Importing task history…</Text>}
        {choices(VIEWS, view, setView, 'Notification view')}
        {choices([[null, 'All projects'], ...(tasks.data?.projects ?? []).map((p) => [p.id, p.name] as [string, string])], project, (id) => { setProject(id); setTask(null); }, 'Project filter')}
        {choices([[null, 'All tasks'], ...(tasks.data?.sessions ?? []).filter((s) => !project || s.project_id === project).map((s) => [s.id, s.auto_title || s.title || 'Task'] as [string, string])], task, setTask, 'Task filter')}
        {choices(KINDS, kind, setKind, 'Notification type')}{choices(STATUSES, status, setStatus, 'Action status')}
        {page && <View style={styles.actions}>{POLICIES.map(([value, label]) => <AutomationButton key={value} label={`${page.notificationPolicy === value ? '✓ ' : ''}${label}`} disabled={!fresh || busy} onPress={() => policy(value)} />)}</View>}
        {!fresh && page && <Text style={[styles.body, { color: theme.textSecondary }]}>Saved history · refresh to use controls</Text>}
        {(message || query.error) && <Text accessibilityRole="alert" style={[styles.body, { color: theme.danger }]}>{message ?? query.error?.message}</Text>}
        {busy && <ActivityIndicator color={theme.accent} accessibilityLabel="Updating notification" />}
      </View>}
      ListEmptyComponent={<Text style={[styles.empty, { color: theme.textSecondary }]}>{query.isFetching ? 'Loading…' : 'No notifications match these filters.'}</Text>}
      ListFooterComponent={<View style={styles.actions}>
        {!!previous.length && <AutomationButton label="Previous" disabled={busy || query.isFetching} onPress={() => { setAfter(previous.at(-1) ?? null); setPrevious(previous.slice(0, -1)); }} />}
        {page?.nextCursor != null && <AutomationButton label="Next" disabled={busy || query.isFetching} onPress={() => { setPrevious([...previous, after]); setAfter(page.nextCursor); }} />}
      </View>}
      renderItem={({ item: entry }) => <View style={[styles.row, { borderBottomColor: theme.border }]}>
        <Text style={[styles.title, { color: theme.text }]}>{entry.title}</Text>
        <Text style={[styles.body, { color: theme.textSecondary }]}>{projectNames.get(entry.projectId ?? '') ?? 'Project'} · {routineTime(entry.occurredAt)} · {entry.state.archived ? 'Archived' : entry.state.acknowledged ? 'Acknowledged' : entry.state.read ? 'Read' : 'Unread'}{entry.needsAction ? ' · Needs action' : ''}</Text>
        <Text selectable style={[styles.body, { color: theme.text }]}>{entry.detail}</Text>
        <View style={styles.actions}>
          <AutomationButton label={entry.needsAction ? 'Review source' : 'Open source'} disabled={!fresh || busy} onPress={() => open(entry)} />
          <AutomationButton label={entry.state.read ? 'Mark unread' : 'Mark read'} disabled={!fresh || busy} onPress={() => update(entry, entry.state.read ? 'markUnread' : 'markRead')} />
          {!entry.state.acknowledged && <AutomationButton label="Acknowledge" disabled={!fresh || busy} onPress={() => update(entry, 'acknowledge')} />}
          <AutomationButton label={entry.state.archived ? 'Restore' : 'Archive'} disabled={!fresh || busy} onPress={() => update(entry, entry.state.archived ? 'restore' : 'archive')} />
        </View>
      </View>} />
  </View>;
}
const styles = StyleSheet.create({
  screen: { flex: 1 }, header: { paddingHorizontal: 20, gap: 12, paddingBottom: 20 },
  filters: { gap: 6 }, filter: { minHeight: 44, justifyContent: 'center', paddingHorizontal: 12, paddingVertical: 10, borderRadius: 8 },
  title: { fontSize: 17, fontWeight: '600', lineHeight: 24 }, body: { fontSize: 14, lineHeight: 21 },
  row: { paddingHorizontal: 20, paddingVertical: 20, gap: 10, borderBottomWidth: StyleSheet.hairlineWidth },
  actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, paddingHorizontal: 0 }, empty: { padding: 24, fontSize: 15, lineHeight: 22 },
});
