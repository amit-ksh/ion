import { router } from 'expo-router';
import { useEffect, useMemo, useState } from 'react';
import { AppState, Pressable, StyleSheet, Text, View, useWindowDimensions } from 'react-native';

import { AppSymbol } from './app-symbol';
import { ProviderIcon } from './provider-icon';
import { StatusPill, TicketCard, ticketStatus } from './ticket-card';
import { Radius } from '@/constants/theme';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import { useRuntime } from '@/lib/runtime-context';
import { displaySessionTitle, relativeSessionTime, sessionTimestamp } from '@/lib/session-presentation';
import { railSessions, sessionTicketState, ticketNumber } from '@/lib/ticket';

/** Most tickets the home rail shows; the task drawer and Work keep the full history. */
const RAIL_LIMIT = 8;

/** One shared clock for the rail's time stamps, parked while the app is hidden. */
function useMinuteClock() {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    const update = (state: string | null) => {
      clearInterval(timer);
      if (state === 'active' || state === null) {
        setNow(Date.now());
        timer = setInterval(() => setNow(Date.now()), 30_000);
      }
    };
    update(AppState.currentState);
    const subscription = AppState.addEventListener('change', update);
    return () => { clearInterval(timer); subscription.remove(); };
  }, []);
  return now;
}

/** Home's live rail: what is working, what waits on the user, what is done. */
export function LiveWork() {
  const theme = useTheme();
  const taskState = useTaskState();
  const runtime = useRuntime();
  const { phase } = useDaemon();
  const now = useMinuteClock();
  const { width } = useWindowDimensions();
  const columns = width >= 600 ? 2 : 1;
  const pending = (id: string) => Boolean(runtime.permissions[id] || runtime.userInputs[id]);
  const sessions = useMemo(
    () => railSessions(taskState.data?.sessions ?? [], RAIL_LIMIT,
      session => Boolean(runtime.permissions[session.id] || runtime.userInputs[session.id])),
    [taskState.data?.sessions, runtime.permissions, runtime.userInputs],
  );
  const projects = useMemo(() => new Map(taskState.data?.projects.map(project => [project.id, project.name])), [taskState.data?.projects]);
  const waiting = sessions.filter(session => pending(session.id) || session.status === 'waiting').length;

  return (
    <View style={styles.rail}>
      <View style={styles.heading}>
        <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>Live work</Text>
        {waiting > 0 && <StatusPill {...ticketStatus('needsYou')} label={`${waiting} Needs you`} />}
        <View style={styles.grow} />
        <Pressable accessibilityRole="button" accessibilityLabel="All tasks" onPress={() => router.push('/work')}
          style={({ pressed }) => [styles.all, { backgroundColor: pressed ? theme.overlayStrong : 'transparent' }]}>
          <Text style={[styles.allLabel, { color: theme.textSecondary }]}>All tasks</Text>
          <AppSymbol name={{ ios: 'chevron.right', android: 'chevron_right', web: 'chevron_right' }} size={16} tintColor={theme.textTertiary} />
        </Pressable>
      </View>
      {sessions.length === 0 ? (
        <View style={[styles.empty, { backgroundColor: theme.ticket, borderColor: theme.ticketBorder }]}>
          <AppSymbol name={{ ios: 'list.bullet', android: 'receipt_long', web: 'receipt_long' }} size={20} tintColor={theme.textTertiary} />
          <Text style={[styles.emptyText, { color: theme.textSecondary }]}>
            {!taskState.isPending
              ? 'Every task you start lands here as a ticket, showing who is on it and what needs you.'
              : phase === 'connected' ? 'Loading tasks…' : 'Your tasks show here once your desktop is connected.'}
          </Text>
        </View>
      ) : (
        <View style={styles.grid}>
          {sessions.map(session => {
            const needsYou = pending(session.id);
            const state = needsYou ? 'needsYou' : sessionTicketState(session);
            return (
              <View key={session.id} style={{ width: columns === 1 ? '100%' : '48.9%' }}>
                <TicketCard
                  number={ticketNumber(session.id)}
                  time={relativeSessionTime(sessionTimestamp(session), now)}
                  title={displaySessionTitle(session)}
                  mark={<ProviderIcon provider={session.provider} size={14} />}
                  station={projects.get(session.project_id) ?? 'Project'}
                  status={ticketStatus(state)}
                  needsYou={state === 'needsYou'}
                  onPress={() => router.push({ pathname: '/session/[id]', params: { id: session.id } })}
                />
              </View>
            );
          })}
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  rail: { gap: 10 },
  heading: { alignItems: 'center', flexDirection: 'row', gap: 10, paddingLeft: 4 },
  title: { fontSize: 16, fontWeight: '600', lineHeight: 22 },
  grow: { flex: 1 },
  all: { alignItems: 'center', borderRadius: Radius.pill, flexDirection: 'row', gap: 2, minHeight: 48, paddingLeft: 12, paddingRight: 6 },
  allLabel: { fontSize: 14, fontWeight: '500' },
  empty: { alignItems: 'center', borderRadius: Radius.medium, borderWidth: StyleSheet.hairlineWidth, flexDirection: 'row', gap: 12, minHeight: 88, paddingHorizontal: 18, paddingVertical: 16 },
  emptyText: { flex: 1, fontSize: 14, lineHeight: 20 },
  grid: { flexDirection: 'row', flexWrap: 'wrap', gap: 10 },
});
