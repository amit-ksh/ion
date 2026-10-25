import type { AgentAccess, AgentProfile, ControlSnapshot, ProviderKind, Team } from '@waku/client';
import * as Crypto from 'expo-crypto';
import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { Alert, ScrollView, KeyboardAvoidingView, Platform, StyleSheet, Text, View } from 'react-native';
import { AutomationChoice, AutomationField, AutomationToggle } from '@/components/automation-form-controls';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { ModelPickerSheet } from '@/components/session-option-sheets';
import { EditorFooter, WorkbenchButton, WorkbenchCard, WorkbenchState } from '@/components/workbench';
import { useWorkspaceDefinition } from '@/hooks/use-workspace-definition';
import { useTheme } from '@/hooks/use-theme';
import { providerLabel } from '@/lib/session-presentation';
import { MOBILE_HARNESSES, mobileHarness } from '@/lib/mobile-capabilities';
import { teamWithReport } from '@/lib/org-chart';

const initialAccess: AgentAccess = { workspaceRead: true, workspaceWrite: false, shell: false, network: false, appActions: [] };
const harnesses = MOBILE_HARNESSES;
export default function AgentEditor() {
  // `parent` (and its `team`) arrive from the org chart's "New subagent".
  const { id, parent, team } = useLocalSearchParams<{ id?: string; parent?: string; team?: string }>();
  const state = useWorkspaceDefinition(id);
  const theme = useTheme();
  const header = useScreenHeaderInset();
  const hydrated = useRef('');
  const [draftId] = useState(Crypto.randomUUID);
  const [name, setName] = useState('');
  const [expertise, setExpertise] = useState('');
  const [instructions, setInstructions] = useState('');
  const [output, setOutput] = useState('');
  const [provider, setProvider] = useState<ProviderKind | null>(null);
  const [model, setModel] = useState<string | null>(null);
  const [environment, setEnvironment] = useState<'local' | 'nebiusSandbox'>('local');
  const [target, setTarget] = useState('');
  const [parallel, setParallel] = useState('1');
  const [access, setAccess] = useState(initialAccess);
  const [picker, setPicker] = useState(false);
  const original = state.data?.records.find((record): record is AgentProfile & { kind: 'agent' } => record.kind === 'agent' && record.id === id);
  const manager = !id && parent ? state.data?.records.find((record): record is AgentProfile & { kind: 'agent' } => record.kind === 'agent' && record.id === parent) : undefined;
  const managerTeam = manager && team ? state.data?.records.find((record): record is Team & { kind: 'team' } => record.kind === 'team' && record.id === team) : undefined;
  function hydrateDraft(snapshot: ControlSnapshot) {
    const saved = snapshot.records.find((record): record is AgentProfile & { kind: 'agent' } => record.kind === 'agent' && record.id === id);
    hydrated.current = `${state.scope}:${id ?? 'new'}`;
    state.bindDraftRevision(snapshot.revision);
    setName(saved?.name ?? ''); setExpertise(saved?.expertise ?? ''); setInstructions(saved?.instructions ?? '');
    const lead = !id && parent ? snapshot.records.find((record): record is AgentProfile & { kind: 'agent' } => record.kind === 'agent' && record.id === parent) : undefined;
    // A subagent starts on its manager's harness and model.
    setOutput(saved?.expectedOutput ?? ''); setProvider(saved?.provider ?? (mobileHarness(lead?.provider ?? null) ? lead!.provider : null)); setModel(saved?.model ?? lead?.model ?? null);
    setEnvironment(saved?.execution.environment ?? 'local'); setTarget(saved?.execution.sandboxTarget ?? '');
    setAccess(saved?.access ?? initialAccess); setParallel(String(saved?.maxParallelTasks ?? 1));
  }
  useEffect(() => {
    if (!state.fresh || !state.data || hydrated.current === `${state.scope}:${id ?? 'new'}`) return;
    hydrateDraft(state.data);
  }, [state.fresh, state.scope, id, original]);
  const maxParallel = Number(parallel);
  const valid = name.trim() && instructions.trim() && mobileHarness(provider) && Number.isInteger(maxParallel) && maxParallel >= 1 && maxParallel <= 16
    && (!access.workspaceWrite || access.workspaceRead) && (environment === 'local' || environment === original?.execution.environment);
  async function save() {
    if (!valid || !provider) return;
    const saved = await state.save({ kind: 'agent', id: id ?? draftId, name: name.trim(), expertise: expertise.trim(),
      instructions: instructions.trim(), expectedOutput: output.trim(), provider, model, maxParallelTasks: maxParallel,
      execution: { environment, sandboxTarget: environment === 'local' ? null : target.trim(), localFallback: original?.execution.localFallback ?? false }, access });
    if (saved && manager) await joinManager(id ?? draftId, manager);
    if (saved) router.back();
  }
  /** Place the new agent under its manager: in the manager's team, or a new
   * "<Manager>'s team" led by the manager when it is on no team. */
  async function joinManager(childId: string, lead: AgentProfile) {
    const client = state.daemon.client;
    const where = team ? managerTeam?.name ?? 'its team' : `${lead.name}'s team`;
    try {
      if (!client) throw new Error('Reconnect to your desktop.');
      const response = await client.request({ type: 'loadWorkspaceControl' });
      if (response.type !== 'workspaceControl') throw new Error('Could not load the workspace.');
      const current = team ? response.snapshot.records.find((record): record is Team & { kind: 'team' } => record.kind === 'team' && record.id === team) : null;
      if (team && !current) throw new Error('The team is no longer available.');
      const record = teamWithReport({ team: current ?? null, parent: lead, childId, newTeamId: Crypto.randomUUID() });
      if (!record) throw new Error('The team is full (32 agents) or already includes this agent.');
      const saved = await client.request({ type: 'saveWorkspaceRecord', expectedRevision: response.snapshot.revision, record });
      if (saved.type !== 'workspaceControl') throw new Error('The team update was not confirmed.');
    } catch (cause) {
      Alert.alert('Agent saved', `It could not join ${where}: ${cause instanceof Error ? cause.message : String(cause)} Add it from the org chart.`);
    }
  }
  if (!state.data) return <WorkbenchState title={state.query.isError ? 'Could not load agent' : 'Loading agent…'}
    detail={state.query.error?.message}><WorkbenchButton label="Retry" onPress={() => void state.query.refetch()} /></WorkbenchState>;
  if (id && !original) return <WorkbenchState title="Agent unavailable" detail="It may have been removed on another device." />;
  return <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} style={{ flex: 1, backgroundColor: theme.background }}><ScrollView keyboardShouldPersistTaps="handled" style={{ flex: 1 }} contentContainerStyle={[styles.content, { paddingTop: header + 16 }]}>
    <ConnectionBanner />
    <View style={styles.heading}>
      <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>{id ? 'Edit agent' : manager ? 'New subagent' : 'New agent'}</Text>
      {manager && <Text style={[styles.note, { color: theme.textSecondary }]}>
        Reports to {manager.name} in {managerTeam ? managerTeam.name : team ? 'its team' : `a new team, ${manager.name}'s team`}
      </Text>}
    </View>
    <WorkbenchCard>
      <AutomationField label="Name" value={name} onChangeText={setName} maxLength={160} editable={!state.busy} />
      <AutomationField label="Expertise" value={expertise} onChangeText={setExpertise} editable={!state.busy} />
      <AutomationField label="Instructions (required)" value={instructions} onChangeText={setInstructions} multiline editable={!state.busy} />
      <AutomationField label="Expected output" value={output} onChangeText={setOutput} editable={!state.busy} />
    </WorkbenchCard>
    <WorkbenchCard>
      <Text style={[styles.section, { color: theme.text }]}>Execution</Text>
      <AutomationChoice label="Harness" value={provider ?? ''} disabled={state.busy}
        options={[{ value: '', label: 'Choose harness' }, ...harnesses.map(value => ({ value, label: providerLabel(value) }))]}
        onChange={value => { setProvider(value as ProviderKind || null); setModel(null); }} />
      <WorkbenchButton label={`Model: ${model ?? 'Harness default'}`} onPress={() => setPicker(true)} disabled={!mobileHarness(provider) || !state.fresh || state.busy} />
      <AutomationChoice label="Environment" value={environment} onChange={value => setEnvironment(value as typeof environment)} disabled={state.busy}
        options={[{ value: 'local', label: 'Paired desktop' },
          ...(original?.execution.environment === 'nebiusSandbox' ? [{ value: 'nebiusSandbox', label: 'Nebius sandbox' }] : [])]} />
      {environment !== 'local' && <Text style={{ color: theme.textSecondary }}>Runs in a Nebius sandbox. Sandbox targets are chosen on desktop.</Text>}
      <AutomationField label="Parallel tasks (1–16)" value={parallel} onChangeText={setParallel} keyboardType="number-pad" editable={!state.busy} />
    </WorkbenchCard>
    <WorkbenchCard>
      <Text style={[styles.section, { color: theme.text }]}>Access</Text>
      {(['workspaceRead', 'workspaceWrite', 'shell', 'network'] as const).map(key => <AutomationToggle key={key}
        label={{ workspaceRead: 'Read workspace', workspaceWrite: 'Write workspace', shell: 'Run shell commands', network: 'Network access' }[key]}
        value={access[key]} disabled={state.busy} onChange={value => setAccess({ ...access, [key]: value })} />)}
      {access.workspaceWrite && !access.workspaceRead && <Text style={{ color: theme.warning }}>Write access also requires read access.</Text>}
      {access.appActions.length > 0 && <Text style={{ color: theme.textSecondary }}>Existing app action grants are retained. Manage individual grants on your desktop.</Text>}
    </WorkbenchCard>
    {state.error && <Text accessibilityLiveRegion="polite" style={{ color: theme.danger }}>{state.error}</Text>}
    {state.conflict && <Text accessibilityLiveRegion="polite" style={{ color: theme.warning }}>Workspace changed. Reload from desktop to review it. Your draft is kept until you reload.</Text>}
    {state.conflict && <WorkbenchButton label="Reload from desktop" disabled={state.busy || state.query.isFetching} onPress={() => void state.reloadDraft(hydrateDraft)} />}
  </ScrollView>
    <EditorFooter busy={state.busy} disabled={!valid || !state.fresh || state.busy || state.conflict} onSave={() => void save()} onClose={() => router.back()} />
    <ModelPickerSheet visible={picker} onDismiss={() => setPicker(false)} provider={provider} model={model} providers={harnesses}
      onApply={selection => { setProvider(selection.provider); setModel(selection.model); setPicker(false); }} />
  </KeyboardAvoidingView>;
}
const styles = StyleSheet.create({ content: { width: '100%', maxWidth: 800, alignSelf: 'center', padding: 16, gap: 20, paddingBottom: 32 }, heading: { gap: 6 }, title: { fontSize: 26, fontWeight: '600' }, note: { fontSize: 15, lineHeight: 22 }, section: { fontSize: 17, fontWeight: '600' }, actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8 } });
