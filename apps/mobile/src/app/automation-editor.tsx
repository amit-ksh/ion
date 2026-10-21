import AsyncStorage from '@react-native-async-storage/async-storage';
import type { ControlSnapshot, RoutineSchedulePreview, TeamRunLimits, WorkflowStage, WorkflowTarget, WorkflowTransition } from '@waku/client';
import * as Crypto from 'expo-crypto';
import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { ActivityIndicator, Alert, KeyboardAvoidingView, Platform, ScrollView, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AutomationButton, AutomationRefresh } from '@/components/automation-controls';
import { AutomationChoice, AutomationField, AutomationToggle } from '@/components/automation-form-controls';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTheme } from '@/hooks/use-theme';
import { clearDefinitionDraft, cloneDefinition, definitionDraftKey, newDefinition, newStage, persistDefinitionDraft, readDefinitionDraft, rebaseDefinitionDraft, removeWorkflowStage, saveDefinitionDraft, type AutomationDefinition, type DefinitionDraft } from '@/lib/automation-editor';

const formats = ['html', 'md', 'txt', 'csv', 'json', 'pdf', 'svg', 'xml', 'yaml', 'yml', 'docx', 'xlsx', 'pptx', 'png'];
const defaultLimits: TeamRunLimits = { maxWorkItems: 64, maxParallelTasks: 4, maxDepth: 8, maxAttempts: 2, maxModelCalls: 64,
  maxTokens: 2000000, requireChildReview: true, maxReworkRounds: 2, timeoutSeconds: 3600 };
export default function AutomationEditorScreen() {
  const params = useLocalSearchParams<{ id?: string; kind?: string; projectId?: string }>();
  const id = typeof params.id === 'string' ? params.id : null;
  const kind = params.kind === 'routine' ? 'routine' : 'workflow'; const slot = id ?? `new-${kind}`;
  const state = useAutomationQuery(['edit-definition', slot], async (client) => {
    const [definitions, tasks] = await Promise.all([client.request({ type: 'loadWorkspaceControl' }), client.request({ type: 'loadTaskState' })]);
    if (definitions.type !== 'workspaceControl' || tasks.type !== 'taskState') throw new Error('Refresh to load definitions and projects.');
    return { snapshot: definitions.snapshot, projects: tasks.projects };
  }, { poll: false });
  const owner = `${state.daemon.activeProfile?.id}:${state.daemon.activeProfile?.address}`;
  return <Editor key={`${owner}:${slot}`} owner={owner} slot={slot} id={id} kind={kind} projectId={params.projectId} state={state} />;
}
function Editor({ owner, slot, id, kind, projectId, state }: {
  owner: string; slot: string; id: string | null; kind: 'workflow' | 'routine'; projectId?: string;
  state: ReturnType<typeof useAutomationQuery<{ snapshot: ControlSnapshot; projects: { id: string; name: string }[] }>>;
}) {
  const theme = useTheme(); const headerInset = useScreenHeaderInset(); const insets = useSafeAreaInsets();
  const [draft, setDraft] = useState<DefinitionDraft | null>(null);
  const [reload, setReload] = useState(0);
  const [formGeneration, setFormGeneration] = useState(0);
  const [error, setError] = useState<string | null>(null); const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState<RoutineSchedulePreview | null>(null); const [limitsOpen, setLimitsOpen] = useState(false);
  const [invalid, setInvalid] = useState<Record<number, boolean>>({});
  const [routineTarget, setRoutineTarget] = useState<'team' | 'workflow'>('team');
  const initialized = useRef(false); const mounted = useRef(true); const locked = useRef(false); const confirmedSaved = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null); const editVersion = useRef(0);
  const latestDraft = useRef(draft); latestDraft.current = draft;
  const epoch = useRef({ client: state.daemon.client, connections: state.daemon.connections, visible: state.visible, value: 0 });
  if (epoch.current.client !== state.daemon.client || epoch.current.connections !== state.daemon.connections || epoch.current.visible !== state.visible) {
    epoch.current = { client: state.daemon.client, connections: state.daemon.connections, visible: state.visible, value: epoch.current.value + 1 };
  }
  const available = useRef(state.fresh && state.visible); available.current = state.fresh && state.visible;
  const canAct = available.current && !busy; const key = definitionDraftKey(owner, slot);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false; if (timer.current) clearTimeout(timer.current);
      if (latestDraft.current && !confirmedSaved.current) void persistDefinitionDraft(AsyncStorage, key, latestDraft.current).catch(() => {});
    };
  }, [key]);
  useEffect(() => {
    if (!state.fresh || !state.data || initialized.current) return;
    initialized.current = true; const data = state.data;
    void AsyncStorage.getItem(key).then((raw) => {
      if (!mounted.current) return;
      if (raw) { const saved = readDefinitionDraft(raw); if (id && saved.record.id !== id) throw new Error('This saved draft belongs to another definition. It has been kept.');
        setDraft(saved); setRoutineTarget(saved.record.kind === 'routine' && saved.record.workflowId ? 'workflow' : 'team'); return; }
      const record = id ? data.snapshot.records.find((r): r is AutomationDefinition => r.id === id && (r.kind === 'workflow' || r.kind === 'routine')) : null;
      if (id && !record) throw new Error('This definition no longer exists. Return to Workflows & routines.');
      const project = data.projects.find((p) => p.id === projectId)?.id ?? data.projects[0]?.id;
      if (!record && !project) throw new Error('Add a project on desktop before creating an automation.');
      const team = data.snapshot.records.find((r) => r.kind === 'team');
      setDraft({ version: 1, revision: data.snapshot.revision, baseline: record ? cloneDefinition(record) : null,
        record: record ?? newDefinition(kind, Crypto.randomUUID(), project!, team?.id ?? null) });
      setRoutineTarget(record?.kind === 'routine' && record.workflowId ? 'workflow' : 'team');
    }).catch((cause) => { if (mounted.current) setError(message(cause)); });
  }, [state.fresh, state.data, key, id, kind, projectId, reload]);
  useEffect(() => {
    if (!draft || locked.current || confirmedSaved.current) return;
    const version = editVersion.current;
    timer.current = setTimeout(() => { timer.current = null; void persistDefinitionDraft(AsyncStorage, key, draft).catch((cause) => {
      if (mounted.current && editVersion.current === version) setError(`Draft could not be saved on this phone: ${message(cause)}`);
    }); }, 350);
    return () => { if (timer.current) clearTimeout(timer.current); timer.current = null; };
  }, [draft, key]);
  function update(record: AutomationDefinition) { if (!mounted.current || !available.current || locked.current || !draft) return; confirmedSaved.current = false; editVersion.current++; setError(null); setPreview(null); setDraft({ ...draft, record }); }
  async function action(operation: (alive: () => boolean) => Promise<void>, expectedEpoch = epoch.current.value) {
    if (!mounted.current || !available.current || locked.current || expectedEpoch !== epoch.current.value) return;
    const started = epoch.current.value; const alive = () => mounted.current && available.current && epoch.current.value === started;
    locked.current = true; setBusy(true); setError(null); if (timer.current) clearTimeout(timer.current); timer.current = null;
    try { await operation(alive); } catch (cause) { if (alive()) setError(message(cause)); }
    finally { locked.current = false; if (mounted.current) setBusy(false); }
  }
  function save() {
    const client = state.daemon.client; if (!client || !draft) return;
    const submitted = draft;
    void action(async (alive) => { await saveDefinitionDraft(client, AsyncStorage, key, submitted); confirmedSaved.current = true;
      if (alive()) { latestDraft.current = null; setDraft(null); router.back(); }
    });
  }
  function reviewLatest() {
    if (!draft) return;
    void action(async (alive) => {
      const refreshed = await state.query.refetch(); const snapshot = refreshed.data?.value.snapshot;
      if (!alive() || !snapshot || refreshed.data?.client !== state.daemon.client) throw new Error('Reconnect and refresh before reviewing changes.');
      const next = rebaseDefinitionDraft(draft, snapshot);
      await persistDefinitionDraft(AsyncStorage, key, next); if (alive()) setDraft(next);
    });
  }
  function loadLatest() {
    const authorization = epoch.current.value;
    Alert.alert('Replace this phone draft?', 'Your unsaved edits will be replaced by the latest saved definition.', [
      { text: 'Keep draft', style: 'cancel' }, { text: 'Load latest', onPress: () => void action(async (alive) => {
        const refreshed = await state.query.refetch(); const data = refreshed.data?.value;
        if (!alive() || !data || refreshed.data?.client !== state.daemon.client) throw new Error('Refresh before replacing your draft.');
        const record = data.snapshot.records.find((r): r is AutomationDefinition => r.id === (draft?.record.id ?? id) && (r.kind === 'workflow' || r.kind === 'routine'));
        if (!record) throw new Error('No saved definition was found. Your draft has been kept.');
        const next: DefinitionDraft = { version: 1, revision: data.snapshot.revision, record, baseline: cloneDefinition(record) };
        await persistDefinitionDraft(AsyncStorage, key, next); if (alive()) { setDraft(next); setPreview(null); setInvalid({}); setFormGeneration((v) => v + 1); setRoutineTarget(record.kind === 'routine' && record.workflowId ? 'workflow' : 'team'); }
      }, authorization) },
    ]);
  }
  function discardLocalDraft() {
    const authorization = epoch.current.value;
    Alert.alert('Discard this phone draft?', 'Only the local draft is removed. If an earlier save was interrupted, check your saved definitions before creating another.', [
      { text: 'Keep draft', style: 'cancel' }, { text: 'Discard local draft', style: 'destructive', onPress: () => void action(async (alive) => {
        await clearDefinitionDraft(AsyncStorage, key);
        if (alive()) { latestDraft.current = null; setDraft(null); initialized.current = false; setError(null); setReload((v) => v + 1); }
      }, authorization) },
    ]);
  }
  function schedulePreview() {
    const client = state.daemon.client; const record = draft?.record; if (!client || record?.kind !== 'routine') return;
    void action(async (alive) => { const response = await client.request({ type: 'previewRoutineSchedule', request: { intervalMinutes: record.intervalMinutes, calendar: record.calendar ?? null } });
      if (response.type !== 'routineSchedulePreview') throw new Error('Refresh and try the schedule preview again.');
      if (alive()) setPreview(response.preview);
    });
  }
  const record = draft?.record; const disabled = !canAct;
  const teams = state.data?.snapshot.records.filter((r) => r.kind === 'team').map((r) => ({ value: r.id, label: r.name })) ?? [];
  const projects = state.data?.projects.map((p) => ({ value: p.id, label: p.name })) ?? [];
  function stage(index: number, changes: Partial<WorkflowStage>) {
    if (record?.kind !== 'workflow') return;
    update({ ...record, stages: record.stages.map((s, i) => i === index ? { ...s, ...changes } : s) });
  }
  function limit(key: keyof TeamRunLimits, value: number | boolean | null) {
    if (record?.kind === 'workflow') update({ ...record, limits: { ...defaultLimits, ...record.limits, [key]: value } });
  }
  return <KeyboardAvoidingView style={[styles.screen, { backgroundColor: theme.background }]} behavior={Platform.OS === 'ios' ? 'padding' : undefined}>
    <ScrollView keyboardShouldPersistTaps="handled" automaticallyAdjustKeyboardInsets contentContainerStyle={[styles.content, { paddingTop: headerInset + 12, paddingBottom: insets.bottom + 24 }]}>
      <ConnectionBanner /><AutomationRefresh refreshing={busy || state.query.isFetching} onRefresh={() => void state.query.refetch()} />
      <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>{id ? 'Edit' : 'New'} {record?.kind ?? kind}</Text>
      {!state.fresh && <Text style={[styles.body, { color: theme.textSecondary }]}>Connect and refresh to edit.</Text>}
      {(error || state.query.error) && <Text accessibilityRole="alert" style={[styles.body, { color: theme.danger }]}>{error ?? state.query.error?.message}</Text>}
      {busy && <ActivityIndicator color={theme.accent} accessibilityLabel="Updating definition" />}
      {!record && error && <AutomationButton label="Discard local draft" disabled={disabled} destructive onPress={discardLocalDraft} />}
      {!record && !error && <ActivityIndicator color={theme.accent} accessibilityLabel="Loading definition" />}
      {record && <>
        <AutomationField label="Name" value={record.name} onChangeText={(name) => update({ ...record, name })} editable={!disabled} maxLength={512} />
        <AutomationChoice label="Project" value={record.projectId} options={projects} disabled={disabled} onChange={(projectId) => update(record.kind === 'routine' ? { ...record, projectId, workflowId: null } : { ...record, projectId })} />
        <AutomationField label={record.kind === 'workflow' ? 'Describe the workflow' : 'Instructions'} value={record.kind === 'workflow' ? record.prompt : record.instructions}
          onChangeText={(value) => update(record.kind === 'workflow' ? { ...record, prompt: value } : { ...record, instructions: value })} editable={!disabled} multiline maxLength={16000} />
        {record.kind === 'workflow' ? <>
          {record.stages.map((s, index) => <View key={`${formGeneration}:${index}`} style={styles.section}>
            <Text accessibilityRole="header" style={[styles.heading, { color: theme.text }]}>Stage {index + 1}</Text>
            <AutomationField label="Stage title" value={s.title} onChangeText={(title) => stage(index, { title })} editable={!disabled} maxLength={512} />
            <AutomationChoice label="Team" value={s.teamId ?? ''} options={[{ value: '', label: 'Choose a team' }, ...teams]} disabled={disabled} onChange={(teamId) => stage(index, { teamId: teamId || null })} />
            <AutomationField label="Stage instructions" value={s.instructions} onChangeText={(instructions) => stage(index, { instructions })} editable={!disabled} multiline maxLength={16000} />
            <AutomationToggle label="Ask for a decision before this stage" value={s.requiresDecision} onChange={(requiresDecision) => stage(index, { requiresDecision })} disabled={disabled} />
            {index > 0 && <AutomationToggle label="Inherit the preceding stage’s code" value={s.inheritWorkspace ?? false} onChange={(inheritWorkspace) => stage(index, { inheritWorkspace })} disabled={disabled} />}
            <Text style={[styles.body, { color: theme.textSecondary }]}>Required documents</Text>
            {formats.map((format) => <AutomationToggle key={format} label={format.toUpperCase()} value={s.requiredFormats?.includes(format) ?? false} disabled={disabled}
              onChange={(checked) => stage(index, { requiredFormats: checked ? [...s.requiredFormats ?? [], format] : s.requiredFormats?.filter((f) => f !== format) ?? [] })} />)}
            <AutomationToggle label="Retry confirmed failures before effects" value={Boolean(s.retry)} disabled={disabled} onChange={(on) => stage(index, { retry: on ? { maxAttempts: 2, delaySeconds: 30 } : undefined })} />
            {s.retry && <>
              <AutomationField label="Maximum attempts, including the first" value={String(s.retry.maxAttempts)} keyboardType="number-pad" editable={!disabled} onChangeText={(v) => stage(index, { retry: { ...s.retry!, maxAttempts: number(v) } })} />
              <AutomationField label="Retry delay in seconds" value={String(s.retry.delaySeconds)} keyboardType="number-pad" editable={!disabled} onChangeText={(v) => stage(index, { retry: { ...s.retry!, delaySeconds: number(v) } })} />
            </>}
            <RouteFields transition={s.transition ?? { type: 'next' }} index={index} stages={record.stages} disabled={disabled} onChange={(transition) => stage(index, { transition })}
              onInvalid={(value) => setInvalid((previous) => previous[index] === value ? previous : { ...previous, [index]: value })} />
            <AutomationButton label="Remove stage" destructive disabled={disabled || record.stages.length <= 1 || Object.entries(invalid).some(([i, value]) => value && Number(i) !== index)} onPress={() => {
              try { update({ ...removeWorkflowStage(record, index), kind: 'workflow' }); setInvalid({}); setFormGeneration((v) => v + 1); } catch (cause) { setError(message(cause)); }
            }} />
          </View>)}
          <AutomationButton label="Add stage" disabled={disabled || record.stages.length >= 24} onPress={() => update({ ...record, stages: [...record.stages, newStage()] })} />
          <AutomationButton label={limitsOpen ? 'Hide workflow limits' : 'Workflow limits'} disabled={disabled} onPress={() => setLimitsOpen(!limitsOpen)} />
          {limitsOpen && <View style={styles.section}>
            <AutomationToggle label="Set workflow limits" value={Boolean(record.limits)} disabled={disabled} onChange={(on) => update({ ...record, limits: on ? { ...defaultLimits } : null })} />
            {record.limits && <>
              {([['maxWorkItems', 'Maximum delegated tasks'], ['maxParallelTasks', 'Concurrent tasks'], ['maxDepth', 'Delegation depth'], ['maxAttempts', 'Attempts per task'], ['maxModelCalls', 'Model calls'], ['maxReworkRounds', 'Rework rounds'], ['timeoutSeconds', 'Timeout in seconds']] as const).map(([key, label]) =>
                <AutomationField key={key} label={label} value={String(record.limits![key])} keyboardType="number-pad" editable={!disabled} onChangeText={(v) => limit(key, number(v))} />)}
              <AutomationField label="Token limit (blank for no workflow token cap)" value={record.limits.maxTokens == null ? '' : String(record.limits.maxTokens)} keyboardType="number-pad" editable={!disabled} onChangeText={(v) => limit('maxTokens', v.trim() ? number(v) : null)} />
              <AutomationToggle label="Require parent review of child work" value={record.limits.requireChildReview} disabled={disabled} onChange={(v) => limit('requireChildReview', v)} />
            </>}
          </View>}
        </> : <>
          <AutomationChoice label="Run" value={routineTarget} options={[{ value: 'team', label: 'A team' }, { value: 'workflow', label: 'A workflow' }]} disabled={disabled}
            onChange={(mode) => { setRoutineTarget(mode as 'team' | 'workflow'); update({ ...record, teamId: null, workflowId: null }); }} />
          <AutomationChoice label={routineTarget === 'workflow' ? 'Workflow' : 'Team'} value={record.workflowId ?? record.teamId ?? ''} disabled={disabled}
            options={[{ value: '', label: 'Choose a target' }, ...(routineTarget === 'workflow' ? state.data?.snapshot.records.flatMap((r) => r.kind === 'workflow' && r.projectId === record.projectId ? [{ value: r.id, label: r.name }] : []) ?? [] : teams)]}
            onChange={(target) => update(routineTarget === 'workflow' ? { ...record, workflowId: target || null, teamId: null } : { ...record, teamId: target || null, workflowId: null })} />
          <ScheduleFields record={record} disabled={disabled} onChange={update} onInvalid={(value) => setInvalid((previous) => previous[-1] === value ? previous : { ...previous, [-1]: value })} />
          <AutomationChoice label="Missed runs" value={record.missedRunPolicy ?? 'skip'} options={[{ value: 'skip', label: 'Skip missed runs' }, { value: 'runLatest', label: 'Run the latest missed occurrence' }]} disabled={disabled} onChange={(v) => update({ ...record, missedRunPolicy: v as 'skip' | 'runLatest' })} />
          <AutomationChoice label="When a previous run is active" value={record.overlapPolicy ?? 'skip'} options={[{ value: 'skip', label: 'Skip overlapping runs' }, { value: 'allow', label: 'Allow overlap within team limits' }]} disabled={disabled} onChange={(v) => update({ ...record, overlapPolicy: v as 'skip' | 'allow' })} />
          <AutomationButton label="Preview schedule" disabled={disabled || Boolean(invalid[-1])} onPress={schedulePreview} />
          {preview && <View style={styles.section}><Text accessibilityRole="header" style={[styles.heading, { color: theme.text }]}>Next occurrences · {preview.timeZone}</Text>
            {preview.fires.map((fire) => <Text key={fire.at} style={[styles.body, { color: theme.text }]}>{fire.display}</Text>)}
          </View>}
          <Text style={[styles.body, { color: theme.textSecondary }]}>Saving pauses this routine. Enable its schedule from Routines when ready.</Text>
        </>}
        <Text style={[styles.body, { color: theme.textSecondary }]}>Existing runs keep their original definitions, teams, permissions and limits.</Text>
        <View style={styles.actions}><AutomationButton label="Save" disabled={disabled || !record.name.trim() || Object.values(invalid).some(Boolean)} onPress={save} />
          <AutomationButton label="Refresh revision" disabled={disabled} onPress={reviewLatest} />
          {id && <AutomationButton label="Load latest definition" disabled={disabled} onPress={loadLatest} />}
        </View>
      </>}
    </ScrollView>
  </KeyboardAvoidingView>;
}
function message(cause: unknown) { return cause instanceof Error ? cause.message : String(cause); }
function number(text: string) { return /^\d+$/.test(text) && Number.isSafeInteger(Number(text)) ? Number(text) : 0; }
function RouteFields({ transition, index, stages, disabled, onChange, onInvalid }: {
  transition: WorkflowTransition; index: number; stages: WorkflowStage[]; disabled: boolean; onChange: (value: WorkflowTransition) => void; onInvalid: (value: boolean) => void;
}) {
  const options = [{ value: 'finish', label: 'Finish workflow' }, ...stages.flatMap((s, i) => i > index ? [{ value: String(i), label: `Stage ${i + 1}: ${s.title || 'Untitled'}` }] : [])];
  const target = (value: string): WorkflowTarget => value === 'finish' ? { type: 'finish' } : { type: 'stage', index: number(value) };
  const selected = (value: WorkflowTarget) => value.type === 'finish' ? 'finish' : String(value.index);
  const scalar = transition.type === 'documentCondition' ? JSON.stringify(transition.equals) : 'true';
  const [equalsText, setEqualsText] = useState(scalar); const [equalsError, setEqualsError] = useState<string | null>(null); const theme = useTheme();
  useEffect(() => { setEqualsText(scalar); setEqualsError(null); onInvalid(false); }, [scalar, transition.type]);
  return <View style={styles.section}>
    <AutomationChoice label="After this stage" value={transition.type} options={[{ value: 'next', label: 'Continue to next stage' }, { value: 'goTo', label: 'Go to a stage or finish' }, { value: 'documentCondition', label: 'Branch on a JSON document value' }]} disabled={disabled}
      onChange={(mode) => onChange(mode === 'next' ? { type: 'next' } : mode === 'goTo' ? { type: 'goTo', target: { type: 'finish' } }
        : { type: 'documentCondition', document: 'result.json', pointer: '/ready', equals: true, onMatch: index + 1 < stages.length ? { type: 'stage', index: index + 1 } : { type: 'finish' }, onMismatch: { type: 'finish' } })} />
    {transition.type === 'goTo' && <AutomationChoice label="Destination" value={selected(transition.target)} options={options} disabled={disabled} onChange={(v) => onChange({ ...transition, target: target(v) })} />}
    {transition.type === 'documentCondition' && <>
      <AutomationField label="Produced JSON document" value={transition.document} editable={!disabled} onChangeText={(document) => onChange({ ...transition, document })} />
      <AutomationField label="Field path (for example /ready)" value={transition.pointer} editable={!disabled} onChangeText={(pointer) => onChange({ ...transition, pointer })} />
      <AutomationField label="Equals (JSON string, number, true, false or null)" value={equalsText} editable={!disabled} autoCapitalize="none" onChangeText={(v) => {
        setEqualsText(v); try { const equals: unknown = JSON.parse(v);
          if (equals !== null && (typeof equals === 'object' || typeof equals === 'number' && !Number.isFinite(equals))) throw new Error();
          onChange({ ...transition, equals: equals as never }); setEqualsError(null); onInvalid(false);
        } catch { setEqualsError('Enter a JSON scalar, such as true or "ready", before saving.'); onInvalid(true); }
      }} />
      {equalsError && <Text accessibilityRole="alert" style={[styles.body, { color: theme.danger }]}>{equalsError}</Text>}
      <AutomationChoice label="When matched" value={selected(transition.onMatch)} options={options} disabled={disabled} onChange={(v) => onChange({ ...transition, onMatch: target(v) })} />
      <AutomationChoice label="Otherwise" value={selected(transition.onMismatch)} options={options} disabled={disabled} onChange={(v) => onChange({ ...transition, onMismatch: target(v) })} />
    </>}
  </View>;
}
function ScheduleFields({ record, disabled, onChange, onInvalid }: {
  record: Extract<AutomationDefinition, { kind: 'routine' }>; disabled: boolean; onChange: (value: AutomationDefinition) => void; onInvalid: (value: boolean) => void;
}) {
  const calendar = record.calendar;
  const days = calendar?.recurrence.type === 'monthly' ? calendar.recurrence.days.join(', ') : '';
  const [daysText, setDaysText] = useState(days); const [daysError, setDaysError] = useState(false); const theme = useTheme();
  useEffect(() => { setDaysText(days); setDaysError(false); onInvalid(false); }, [days, calendar?.recurrence.type]);
  return <View style={styles.section}>
    <AutomationChoice label="Schedule" value={calendar?.recurrence.type ?? 'interval'} disabled={disabled}
      options={[{ value: 'interval', label: 'Every N minutes' }, { value: 'daily', label: 'Daily' }, { value: 'weekly', label: 'Weekly' }, { value: 'monthly', label: 'Monthly' }]}
      onChange={(mode) => onChange({ ...record, calendar: mode === 'interval' ? undefined : { timeZone: calendar?.timeZone ?? 'UTC', hour: calendar?.hour ?? 9, minute: calendar?.minute ?? 0,
        recurrence: mode === 'daily' ? { type: 'daily' } : mode === 'weekly' ? { type: 'weekly', days: [0] } : { type: 'monthly', days: [1] } } })} />
    {!calendar ? <AutomationField label="Interval in minutes" value={String(record.intervalMinutes)} keyboardType="number-pad" editable={!disabled} onChangeText={(v) => onChange({ ...record, intervalMinutes: number(v) })} /> : <>
      <AutomationField label="Time zone (for example Asia/Kolkata)" value={calendar.timeZone} editable={!disabled} autoCapitalize="none" onChangeText={(timeZone) => onChange({ ...record, calendar: { ...calendar, timeZone } })} />
      <AutomationField label="Hour (0–23)" value={String(calendar.hour)} keyboardType="number-pad" editable={!disabled} onChangeText={(v) => onChange({ ...record, calendar: { ...calendar, hour: number(v) } })} />
      <AutomationField label="Minute (0–59)" value={String(calendar.minute)} keyboardType="number-pad" editable={!disabled} onChangeText={(v) => onChange({ ...record, calendar: { ...calendar, minute: number(v) } })} />
      {calendar.recurrence.type === 'weekly' && ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'].map((label, day) => <AutomationToggle key={day} label={label} disabled={disabled}
        value={calendar.recurrence.type === 'weekly' && calendar.recurrence.days.includes(day)} onChange={(checked) => {
          const days = calendar.recurrence.type === 'weekly' ? calendar.recurrence.days : [];
          onChange({ ...record, calendar: { ...calendar, recurrence: { type: 'weekly', days: checked ? [...days, day].sort((a, b) => a - b) : days.filter((d) => d !== day) } } });
        }} />)}
      {calendar.recurrence.type === 'monthly' && <AutomationField label="Days of the month (comma separated, 1–31)" value={daysText} editable={!disabled} keyboardType="numbers-and-punctuation" onChangeText={(v) => {
        setDaysText(v); const parts = v.split(',').map((d) => d.trim()); const next = parts.map(number);
        const valid = parts.every((d) => /^\d+$/.test(d)) && next.every((day) => day >= 1 && day <= 31) && new Set(next).size === next.length;
        setDaysError(!valid); onInvalid(!valid);
        if (valid) onChange({ ...record, calendar: { ...calendar, recurrence: { type: 'monthly', days: next } } });
      }} />}
      {daysError && <Text accessibilityRole="alert" style={[styles.body, { color: theme.danger }]}>Choose unique days from 1 through 31 before saving.</Text>}
    </>}
  </View>;
}
const styles = StyleSheet.create({
  screen: { flex: 1 }, content: { paddingHorizontal: 20, gap: 16, maxWidth: 760, width: '100%', alignSelf: 'center' },
  title: { fontSize: 24, lineHeight: 32, fontWeight: '600' }, heading: { fontSize: 18, lineHeight: 26, fontWeight: '600' },
  body: { fontSize: 15, lineHeight: 23 }, section: { gap: 12, paddingTop: 20, marginTop: 8 },
  actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, paddingTop: 16 },
});
