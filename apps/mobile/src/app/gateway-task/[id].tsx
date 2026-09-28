import * as Crypto from 'expo-crypto';
import { Stack, useFocusEffect, useLocalSearchParams } from 'expo-router';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Alert, FlatList, Pressable, StyleSheet, Text, TextInput, View } from 'react-native';

import { useTheme } from '@/hooks/use-theme';
import {
  enqueueGatewayTaskCommand, gatewayTaskCommand, gatewayTaskEvents, gatewayTasks,
  gatewayTaskInteractions, type GatewayPairing, type GatewayTask, type GatewayTaskCommand,
  type GatewayTaskEvent, type GatewayTaskInteraction,
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
  const [interactions, setInteractions] = useState<GatewayTaskInteraction[]>([]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [stale, setStale] = useState(true);
  const [steer, setSteer] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const [command, setCommand] = useState<GatewayTaskCommand | null>(null);
  const [stopCommand, setStopCommand] = useState<GatewayTaskCommand | null>(null);
  const [stopPending, setStopPending] = useState(false);
  const [pendingInput, setPendingInput] = useState<ControlInput | null>(null);
  const cursor = useRef(0);
  const loading = useRef(false);
  const activeId = useRef(id);
  const commandRef = useRef<GatewayTaskCommand | null>(null);
  const stopCommandRef = useRef<GatewayTaskCommand | null>(null);
  const stopInputRef = useRef<ControlInput | null>(null);
  const pendingRef = useRef<ControlInput | null>(null);

  useEffect(() => {
    let active = true;
    activeId.current = id;
    cursor.current = 0;
    loading.current = false;
    commandRef.current = null;
    stopCommandRef.current = null;
    stopInputRef.current = null;
    pendingRef.current = null;
    setPairing(null);
    setEvents([]);
    setInteractions([]);
    setTask(null);
    setAnswers({});
    setStale(true);
    setSteer('');
    setMessage(null);
    setCommand(null);
    setStopCommand(null);
    setStopPending(false);
    setPendingInput(null);
    void readGatewayPairing().then(async (saved) => {
      if (!active || !saved) return;
      setPairing(saved);
      const cached = await readCachedGatewayTasks(`${saved.baseUrl}:${saved.id}`);
      if (active) setTask(cached.find((item) => item.id === id) ?? null);
      if (active) void refresh(saved);
    }).catch(() => { if (active) setMessage('Could not load gateway pairing.'); });
    return () => { active = false; };
  }, [id]);

  useFocusEffect(useCallback(() => {
    if (!pairing) return;
    const timer = setInterval(() => void refresh(pairing), 4_000);
    return () => clearInterval(timer);
  }, [pairing, id]));

  async function refresh(current = pairing) {
    if (!current || loading.current || !current.grants.includes('taskRead')) return;
    const requestedId = id;
    loading.current = true;
    try {
      const [catalog, feed, pending] = await Promise.all([
        gatewayTasks(current), gatewayTaskEvents(current, cursor.current), gatewayTaskInteractions(current, requestedId),
      ]);
      if (activeId.current !== requestedId) return;
      const currentTask = catalog.tasks.find((item) => item.id === requestedId) ?? null;
      setTask(currentTask);
      setInteractions(currentTask ? pending.filter((item) => item.runtimeId === currentTask.runtimeId) : []);
      if (!currentTask) setEvents([]);
      const isStale = !currentTask?.publisherSeenAt || Date.now() - currentTask.publisherSeenAt > 60_000;
      setStale(isStale);
      if (feed.resetRequired) setEvents([]);
      if (currentTask) setEvents((previous) => [
        ...(feed.resetRequired ? [] : previous),
        ...feed.events.filter((event) => event.taskId === requestedId),
      ].slice(-300));
      cursor.current = feed.nextCursor;
      if (isStale) setMessage('Desktop sync is stale. Task controls are paused.');
      else if (pendingRef.current) await submitPending(current);
      if (activeId.current !== requestedId) return;
      if (!isStale && stopInputRef.current) await submitStop(current);
      if (activeId.current !== requestedId) return;
      if (commandRef.current) {
        const latest = await gatewayTaskCommand(current, commandRef.current.id);
        if (activeId.current !== requestedId) return;
        commandRef.current = latest;
        setCommand(latest);
        if (latest.status === 'applied' || latest.status === 'rejected') {
          setMessage(latest.status === 'applied'
            ? 'Command accepted by the desktop daemon.' : 'Command rejected; the task may have changed.');
        }
      }
      if (stopCommandRef.current) {
        const latest = await gatewayTaskCommand(current, stopCommandRef.current.id);
        if (activeId.current !== requestedId) return;
        stopCommandRef.current = latest;
        setStopCommand(latest);
        if (latest.status === 'applied') setMessage('Desktop daemon accepted the emergency stop.');
        else if (latest.status === 'rejected') setMessage('Stop was rejected; inspect the desktop task state.');
      }
    } catch {
      if (activeId.current !== requestedId) return;
      setStale(true);
      setMessage('Gateway connection unavailable. Showing the last visible task state; controls are paused.');
    } finally { if (activeId.current === requestedId) loading.current = false; }
  }

  async function submitPending(current: GatewayPairing) {
    const input = pendingRef.current;
    if (!input) return;
    try {
      const queued = await enqueueGatewayTaskCommand(current, input);
      if (activeId.current !== input.taskId || pendingRef.current !== input) return;
      commandRef.current = queued;
      setCommand(queued);
      pendingRef.current = null;
      setPendingInput(null);
      setMessage('Command recorded by the gateway. Waiting for the desktop.');
    } catch {
      if (activeId.current !== input.taskId || pendingRef.current !== input) return;
      setMessage('Command status is unknown. Reconnect to reconcile the same request ID.');
      setStale(true);
    }
  }

  async function submitStop(current: GatewayPairing) {
    const input = stopInputRef.current;
    if (!input) return;
    try {
      const queued = await enqueueGatewayTaskCommand(current, input);
      if (activeId.current !== input.taskId || stopInputRef.current !== input) return;
      stopCommandRef.current = queued;
      setStopCommand(queued);
      stopInputRef.current = null;
      setStopPending(false);
      setMessage('Emergency stop recorded. Waiting for the desktop.');
    } catch {
      if (activeId.current !== input.taskId || stopInputRef.current !== input) return;
      setStale(true);
      setMessage('Stop status is unknown. Reconnect to retry the same request ID.');
    }
  }

  function emergencyStop() {
    if (!pairing || !task?.runtimeId || stale || !pairing.grants.includes('taskControl') ||
        stopInputRef.current || (stopCommand?.runtimeId === task.runtimeId &&
          ['queued', 'uncertain', 'applied'].includes(stopCommand.status))) return;
    stopInputRef.current = {
      idempotencyKey: Crypto.randomUUID(), taskId: task.id, runtimeId: task.runtimeId,
      kind: 'stop', payload: {},
    };
    setStopPending(true);
    void submitStop(pairing);
  }

  function submit(kind: 'steer') {
    if (!pairing || !task?.runtimeId || stale || !pairing.grants.includes('taskControl') ||
        pendingInput || (command && ['queued', 'uncertain'].includes(command.status))) return;
    const input: ControlInput = {
      idempotencyKey: Crypto.randomUUID(), taskId: task.id, runtimeId: task.runtimeId,
      kind, payload: { prompt: steer.trim() },
    };
    if (!steer.trim()) return;
    const dispatch = () => {
      pendingRef.current = input;
      setPendingInput(input);
      void submitPending(pairing);
      setSteer('');
    };
    dispatch();
  }

  function submitInteraction(interaction: GatewayTaskInteraction, optionId?: string) {
    if (!pairing || !task?.runtimeId || stale || !pairing.grants.includes('taskControl') ||
        interaction.runtimeId !== task.runtimeId || pendingRef.current) return;
    const payload: Record<string, string> = {
      interactionId: interaction.id, fingerprint: interaction.fingerprint,
    };
    if (interaction.kind === 'permission') {
      if (!optionId || !interaction.options.some((option) => option.id === optionId)) return;
      payload.optionId = optionId;
    } else {
      const items = interaction.questions.map((question) => ({
        questionId: question.id,
        answers: question.multiSelect
          ? (answers[question.id] ?? '').split('\n').map((item) => item.trim()).filter(Boolean)
          : [(answers[question.id] ?? '').trim()].filter(Boolean),
      }));
      if (items.some((item) => item.answers.length === 0)) return;
      payload.answers = JSON.stringify(items);
    }
    const input: ControlInput = {
      idempotencyKey: Crypto.randomUUID(), taskId: task.id, runtimeId: task.runtimeId,
      kind: interaction.kind === 'permission' ? 'permission' : 'answer', payload,
    };
    const dispatch = () => {
      pendingRef.current = input;
      setPendingInput(input);
      void submitPending(pairing);
    };
    const selectedOption = interaction.options.find((option) => option.id === optionId);
    if (interaction.kind === 'permission' && selectedOption?.allow) {
      Alert.alert(`${selectedOption.label}?`, `${interaction.title}\n${interaction.detail}`, [
        { text: 'Cancel', style: 'cancel' }, { text: selectedOption.label, onPress: dispatch },
      ]);
    } else dispatch();
  }

  const canControl = !!pairing?.grants.includes('taskControl') && !!task?.runtimeId && !stale &&
    !pendingInput && !(command && ['queued', 'uncertain'].includes(command.status));
  const canStop = !!pairing?.grants.includes('taskControl') && !!task?.runtimeId && !stale &&
    !stopPending && !(stopCommand?.runtimeId === task?.runtimeId &&
      ['queued', 'uncertain', 'applied'].includes(stopCommand.status));
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
      {task?.computerTarget?.visible && <Text style={[styles.meta, { color: theme.text }]}>
        Computer control target: {task.computerTarget.appName} · {task.computerTarget.windowTitle || 'Untitled window'} · {task.computerTarget.phase}
      </Text>}
      {message && <Text accessibilityRole="alert" style={[styles.meta, { color: theme.textSecondary }]}>{message}</Text>}
      {command && <Text style={[styles.meta, { color: theme.textSecondary }]}>Last command: {command.kind} · {command.status}</Text>}
      {stopCommand && <Text style={[styles.meta, { color: theme.textSecondary }]}>Emergency stop: {stopCommand.status}</Text>}
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
          <Pressable accessibilityRole="button" accessibilityState={{ disabled: !canStop }}
            disabled={!canStop} onPress={emergencyStop}
            style={[styles.button, { borderColor: theme.danger, opacity: canStop ? 1 : 0.5 }]}>
            <Text style={{ color: theme.danger, fontWeight: '600' }}>Emergency stop</Text>
          </Pressable>
        </View>
      )}
      {interactions.map((interaction) => (
        <View key={interaction.id} style={[styles.event, { borderColor: theme.border }]}>
          <Text style={[styles.section, { color: theme.text }]}>{interaction.kind === 'permission' ? 'Provider permission' : 'Provider question'}</Text>
          <Text style={[styles.meta, { color: theme.text }]}>{interaction.title}</Text>
          {!!interaction.detail && <Text selectable style={[styles.eventText, { color: theme.text }]}>{interaction.detail}</Text>}
          <Text selectable style={[styles.meta, { color: theme.textSecondary }]}>Exact review: {interaction.fingerprint}</Text>
          {interaction.options.map((option) => (
            <Pressable key={option.id} accessibilityRole="button" accessibilityState={{ disabled: !canControl }}
              disabled={!canControl} onPress={() => submitInteraction(interaction, option.id)}
              style={[styles.button, { borderColor: option.allow ? theme.accent : theme.danger, opacity: canControl ? 1 : 0.5 }]}>
              <Text style={{ color: option.allow ? theme.accent : theme.danger }}>{option.label}</Text>
            </Pressable>
          ))}
          {interaction.questions.map((question) => (
            <View key={question.id} style={styles.controls}>
              <Text style={[styles.meta, { color: theme.text }]}>{question.header}: {question.question}</Text>
              {question.options.map((option) => (
                <Text key={option.label} style={[styles.meta, { color: theme.textSecondary }]}>{option.label}{option.description ? ` · ${option.description}` : ''}</Text>
              ))}
              <TextInput accessibilityLabel={`Answer ${question.header}`} multiline={question.multiSelect}
                editable={canControl} value={answers[question.id] ?? ''}
                onChangeText={(value) => setAnswers((current) => ({ ...current, [question.id]: value }))}
                placeholder={question.multiSelect ? 'One answer per line' : 'Enter an answer'}
                placeholderTextColor={theme.textTertiary}
                style={[styles.input, { borderColor: theme.borderStrong, color: theme.text, backgroundColor: theme.surface }]} />
            </View>
          ))}
          {interaction.kind === 'question' && <Pressable accessibilityRole="button"
            accessibilityState={{ disabled: !canControl }} disabled={!canControl}
            onPress={() => submitInteraction(interaction)}
            style={[styles.button, { borderColor: theme.accent, opacity: canControl ? 1 : 0.5 }]}>
            <Text style={{ color: theme.accent }}>Send answer</Text>
          </Pressable>}
        </View>
      ))}
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
