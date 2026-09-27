import * as Crypto from 'expo-crypto';
import { Stack, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { Alert, FlatList, Pressable, StyleSheet, Text, TextInput, View } from 'react-native';

import { useTheme } from '@/hooks/use-theme';
import {
  enqueueGatewayTaskCommand, gatewayTaskCommand, gatewayTaskEvents, gatewayTasks,
  type GatewayPairing, type GatewayTask, type GatewayTaskCommand, type GatewayTaskEvent,
} from '@/lib/gateway-client';
import { readGatewayPairing } from '@/lib/gateway-storage';
import { readCachedGatewayTasks } from '@/lib/gateway-cache';

type ControlInput = Parameters<typeof enqueueGatewayTaskCommand>[1];

export default function GatewayTaskScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const theme = useTheme();
  const [pairing, setPairing] = useState<GatewayPairing | null>(null);
  const [task, setTask] = useState<GatewayTask | null>(null);
  const [events, setEvents] = useState<GatewayTaskEvent[]>([]);
  const [stale, setStale] = useState(true);
  const [steer, setSteer] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const [command, setCommand] = useState<GatewayTaskCommand | null>(null);
  const [pendingInput, setPendingInput] = useState<ControlInput | null>(null);
  const cursor = useRef(0);
  const loading = useRef(false);
  const commandRef = useRef<GatewayTaskCommand | null>(null);
  const pendingRef = useRef<ControlInput | null>(null);

  useEffect(() => {
    let active = true;
    void readGatewayPairing().then(async (saved) => {
      if (!active || !saved) return;
      setPairing(saved);
      const cached = await readCachedGatewayTasks(`${saved.baseUrl}:${saved.id}`);
      if (active) setTask(cached.find((item) => item.id === id) ?? null);
      if (active) void refresh(saved);
    }).catch(() => { if (active) setMessage('Could not load gateway pairing.'); });
    return () => { active = false; };
  }, [id]);

  useEffect(() => {
    if (!pairing) return;
    const timer = setInterval(() => void refresh(pairing), 4_000);
    return () => clearInterval(timer);
  }, [pairing, id]);

  async function refresh(current = pairing) {
    if (!current || loading.current || !current.grants.includes('taskRead')) return;
    loading.current = true;
    try {
      const [catalog, feed] = await Promise.all([
        gatewayTasks(current), gatewayTaskEvents(current, cursor.current),
      ]);
      const currentTask = catalog.tasks.find((item) => item.id === id) ?? null;
      setTask(currentTask);
      if (!currentTask) setEvents([]);
      const isStale = !currentTask?.publisherSeenAt || Date.now() - currentTask.publisherSeenAt > 60_000;
      setStale(isStale);
      if (feed.resetRequired) setEvents([]);
      if (currentTask) setEvents((previous) => [
        ...(feed.resetRequired ? [] : previous),
        ...feed.events.filter((event) => event.taskId === id),
      ].slice(-300));
      cursor.current = feed.nextCursor;
      if (isStale) setMessage('Desktop sync is stale. Task controls are paused.');
      else if (pendingRef.current) await submitPending(current);
      if (commandRef.current) {
        const latest = await gatewayTaskCommand(current, commandRef.current.id);
        commandRef.current = latest;
        setCommand(latest);
        if (latest.status === 'applied' || latest.status === 'rejected') {
          setMessage(latest.status === 'applied'
            ? 'Command accepted by the desktop daemon.' : 'Command rejected; the task may have changed.');
        }
      }
    } catch {
      setStale(true);
      setMessage('Gateway connection unavailable. Showing the last visible task state; controls are paused.');
    } finally { loading.current = false; }
  }

  async function submitPending(current: GatewayPairing) {
    const input = pendingRef.current;
    if (!input) return;
    try {
      const queued = await enqueueGatewayTaskCommand(current, input);
      commandRef.current = queued;
      setCommand(queued);
      pendingRef.current = null;
      setPendingInput(null);
      setMessage('Command recorded by the gateway. Waiting for the desktop.');
    } catch {
      setMessage('Command status is unknown. Reconnect to reconcile the same request ID.');
      setStale(true);
    }
  }

  function submit(kind: 'stop' | 'steer') {
    if (!pairing || !task?.runtimeId || stale || !pairing.grants.includes('taskControl') ||
        pendingInput || (command && ['queued', 'uncertain'].includes(command.status))) return;
    const input: ControlInput = {
      idempotencyKey: Crypto.randomUUID(), taskId: task.id, runtimeId: task.runtimeId,
      kind, payload: kind === 'steer' ? { prompt: steer.trim() } : {},
    };
    if (kind === 'steer' && !steer.trim()) return;
    const dispatch = () => {
      pendingRef.current = input;
      setPendingInput(input);
      void submitPending(pairing);
      if (kind === 'steer') setSteer('');
    };
    if (kind === 'stop') {
      Alert.alert('Stop this task?', 'The desktop will stop computer control and cancel the active turn when it receives this command.', [
        { text: 'Keep running', style: 'cancel' },
        { text: 'Stop task', style: 'destructive', onPress: dispatch },
      ]);
    } else dispatch();
  }

  const canControl = !!pairing?.grants.includes('taskControl') && !!task?.runtimeId && !stale &&
    !pendingInput && !(command && ['queued', 'uncertain'].includes(command.status));
  const header = (
    <View style={styles.header}>
      <Stack.Screen options={{ title: task?.title ?? 'Gateway task' }} />
      <Text style={[styles.title, { color: theme.text }]}>{task?.title ?? 'Task unavailable'}</Text>
      <Text style={[styles.meta, { color: theme.textSecondary }]}>
        {task ? `${task.projectName} · ${task.provider} · ${task.status}` : 'Connect to the gateway to load this task.'}
      </Text>
      <Text style={[styles.status, { color: stale ? theme.warning : theme.success }]}>
        {stale ? 'Stale or offline · controls unavailable' : 'Connected to desktop feed'}
      </Text>
      {message && <Text accessibilityRole="alert" style={[styles.meta, { color: theme.textSecondary }]}>{message}</Text>}
      {command && <Text style={[styles.meta, { color: theme.textSecondary }]}>Last command: {command.kind} · {command.status}</Text>}
      {pairing?.grants.includes('taskControl') && (
        <View style={styles.controls}>
          <TextInput accessibilityLabel="Steer task" multiline value={steer} onChangeText={setSteer}
            editable={canControl} placeholder="Message the running task" placeholderTextColor={theme.textTertiary}
            style={[styles.input, { borderColor: theme.borderStrong, color: theme.text, backgroundColor: theme.surface }]} />
          <Pressable accessibilityRole="button" accessibilityState={{ disabled: !canControl || !steer.trim() }}
            disabled={!canControl || !steer.trim()} onPress={() => submit('steer')}
            style={[styles.button, { borderColor: theme.accent, opacity: canControl && steer.trim() ? 1 : 0.5 }]}>
            <Text style={{ color: theme.accent, fontWeight: '600' }}>Steer task</Text>
          </Pressable>
          <Pressable accessibilityRole="button" accessibilityState={{ disabled: !canControl }}
            disabled={!canControl} onPress={() => submit('stop')}
            style={[styles.button, { borderColor: theme.danger, opacity: canControl ? 1 : 0.5 }]}>
            <Text style={{ color: theme.danger, fontWeight: '600' }}>Emergency stop</Text>
          </Pressable>
        </View>
      )}
      <Text style={[styles.section, { color: theme.text }]}>Ordered activity</Text>
    </View>
  );

  return <FlatList style={{ backgroundColor: theme.background }} contentContainerStyle={styles.list}
    data={events} keyExtractor={(event) => String(event.cursor)} ListHeaderComponent={header}
    ListEmptyComponent={<Text style={[styles.meta, { color: theme.textSecondary }]}>No mirrored events yet.</Text>}
    renderItem={({ item }) => (
      <View style={[styles.event, { borderColor: theme.border }]}>
        <Text style={[styles.eventKind, { color: theme.textSecondary }]}>{item.kind.toUpperCase()} · #{item.cursor}</Text>
        <Text selectable style={[item.kind === 'diff' ? styles.code : styles.eventText, { color: theme.text }]}>{item.text}</Text>
      </View>
    )} />;
}

const styles = StyleSheet.create({
  list: { padding: 18, paddingBottom: 70 },
  header: { gap: 8, marginBottom: 14 },
  title: { fontSize: 22, fontWeight: '700' },
  meta: { fontSize: 13, lineHeight: 19 },
  status: { fontSize: 13, fontWeight: '600' },
  controls: { gap: 8, marginTop: 12 },
  input: { borderWidth: 1, borderRadius: 9, minHeight: 70, padding: 12, fontSize: 15 },
  button: { borderWidth: 1, borderRadius: 9, minHeight: 44, alignItems: 'center', justifyContent: 'center' },
  section: { fontSize: 15, fontWeight: '700', marginTop: 16 },
  event: { borderTopWidth: 1, paddingVertical: 12, gap: 5 },
  eventKind: { fontSize: 11, fontWeight: '600' },
  eventText: { fontSize: 14, lineHeight: 21 },
  code: { fontFamily: 'monospace', fontSize: 12, lineHeight: 18 },
});
