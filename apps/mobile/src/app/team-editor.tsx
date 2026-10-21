import type { AgentProfile, ControlSnapshot, Team, TeamMember } from '@waku/client';
import * as Crypto from 'expo-crypto';
import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';
import { AutomationChoice, AutomationField, AutomationToggle } from '@/components/automation-form-controls';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { WorkbenchButton, WorkbenchCard, WorkbenchState } from '@/components/workbench';
import { useWorkspaceDefinition } from '@/hooks/use-workspace-definition';
import { useTheme } from '@/hooks/use-theme';
import { teamDraftProblem } from '@/lib/team-draft';

export default function TeamEditor() {
  const { id } = useLocalSearchParams<{ id?: string }>();
  const state = useWorkspaceDefinition(id);
  const theme = useTheme();
  const header = useScreenHeaderInset();
  const hydrated = useRef('');
  const [draftId] = useState(Crypto.randomUUID);
  const [name, setName] = useState('');
  const [objective, setObjective] = useState('');
  const [members, setMembers] = useState<TeamMember[]>([]);
  const original = state.data?.records.find((record): record is Team & { kind: 'team' } => record.kind === 'team' && record.id === id);
  const agents = useMemo(() => (state.data?.records ?? []).filter((record): record is AgentProfile & { kind: 'agent' } => record.kind === 'agent'), [state.data]);
  const problem = useMemo(() => teamDraftProblem(members, new Set(agents.map(agent => agent.id))), [members, agents]);
  function hydrateDraft(snapshot: ControlSnapshot) {
    const saved = snapshot.records.find((record): record is Team & { kind: 'team' } => record.kind === 'team' && record.id === id);
    hydrated.current = `${state.scope}:${id ?? 'new'}`;
    state.bindDraftRevision(snapshot.revision);
    setName(saved?.name ?? ''); setObjective(saved?.objective ?? ''); setMembers(saved?.members ?? []);
  }
  useEffect(() => {
    if (!state.fresh || !state.data || hydrated.current === `${state.scope}:${id ?? 'new'}`) return;
    hydrateDraft(state.data);
  }, [state.fresh, state.scope, id, original]);
  async function save() {
    if (problem || !name.trim() || !objective.trim()) return;
    if (await state.save({ kind: 'team', id: id ?? draftId, name: name.trim(), objective: objective.trim(), members, ...(original?.limits ? { limits: original.limits } : {}) })) router.back();
  }
  if (!state.data) return <WorkbenchState title={state.query.isError ? 'Could not load team' : 'Loading team…'} detail={state.query.error?.message}>
    <WorkbenchButton label="Retry" onPress={() => void state.query.refetch()} /></WorkbenchState>;
  if (id && !original) return <WorkbenchState title="Team unavailable" detail="It may have been removed on another device." />;
  return <FlatList data={agents} keyExtractor={agent => agent.id} keyboardShouldPersistTaps="handled" style={{ backgroundColor: theme.background }} contentContainerStyle={[styles.content, { paddingTop: header + 16 }]}
    ListHeaderComponent={<View style={styles.group}>
    <ConnectionBanner />
    <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>{id ? 'Edit team' : 'New team'}</Text>
    <WorkbenchCard><AutomationField label="Name" value={name} onChangeText={setName} maxLength={160} editable={!state.busy} />
      <AutomationField label="Objective" value={objective} onChangeText={setObjective} multiline editable={!state.busy} /></WorkbenchCard>
    <View style={styles.sectionHeader}><Text accessibilityRole="header" style={[styles.section, { color: theme.text }]}>Agents & hierarchy</Text>
      <WorkbenchButton label="New agent" onPress={() => router.push('/agent-editor')} disabled={state.busy} /></View>
    </View>}
    ListEmptyComponent={<WorkbenchState title="Create an agent first" detail="Then choose a lead and add agents to this team." />}
    renderItem={({ item: agent }) => {
      const member = members.find(item => item.agentId === agent.id);
      return <WorkbenchCard key={agent.id}>
        <AutomationToggle label={agent.name} value={Boolean(member)} disabled={state.busy || !state.fresh}
          onChange={selected => setMembers(current => selected ? [...current, { agentId: agent.id, parentId: current[0]?.agentId ?? null }]
            : current.filter(item => item.agentId !== agent.id).map(item => item.parentId === agent.id ? { ...item, parentId: null } : item))} />
        {member && <AutomationChoice label={`${agent.name} reports to`} value={member.parentId ?? ''} disabled={state.busy}
          options={[{ value: '', label: 'Team lead' }, ...members.filter(item => item.agentId !== agent.id).map(item => ({ value: item.agentId, label: agents.find(candidate => candidate.id === item.agentId)?.name ?? 'Unavailable agent' }))]}
          onChange={parentId => setMembers(current => current.map(item => item.agentId === agent.id ? { ...item, parentId: parentId || null } : item))} />}
      </WorkbenchCard>;
    }}
    ListFooterComponent={<View style={styles.group}>
    {members.some(member => !agents.some(agent => agent.id === member.agentId)) && <WorkbenchCard>
      <Text style={{ color: theme.warning }}>This team contains an unavailable agent.</Text>
      <WorkbenchButton label="Remove unavailable agents" onPress={() => setMembers(current => current.filter(member => agents.some(agent => agent.id === member.agentId)))} />
    </WorkbenchCard>}
    {problem && members.length > 0 && <Text accessibilityLiveRegion="polite" style={{ color: theme.warning }}>{problem}</Text>}
    {state.error && <Text accessibilityLiveRegion="polite" style={{ color: theme.danger }}>{state.error}</Text>}
    {state.conflict && <Text accessibilityLiveRegion="polite" style={{ color: theme.warning }}>Workspace changed. Reload from desktop to review it. Your draft is kept until you reload.</Text>}
    <View style={styles.actions}><WorkbenchButton label={state.busy ? 'Saving…' : 'Save team'} primary disabled={Boolean(problem) || !name.trim() || !objective.trim() || !state.fresh || state.busy || state.conflict} onPress={() => void save()} />
      <WorkbenchButton label="Reload from desktop" disabled={state.busy || state.query.isFetching} onPress={() => void state.reloadDraft(hydrateDraft)} /></View>
    </View>} />;
}
const styles = StyleSheet.create({ content: { width: '100%', maxWidth: 800, alignSelf: 'center', padding: 16, gap: 16, paddingBottom: 32 }, group: { gap: 16 }, title: { fontSize: 26, fontWeight: '600' }, section: { fontSize: 17, fontWeight: '600' }, sectionHeader: { flexDirection: 'row', flexWrap: 'wrap', alignItems: 'center', justifyContent: 'space-between', gap: 12 }, actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8 } });
