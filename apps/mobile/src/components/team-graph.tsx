import type { AgentProfile, TeamMember } from '@waku/client';
import { useMemo, useState } from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View, useWindowDimensions } from 'react-native';
import { ProviderIcon } from './provider-icon';
import { useTheme } from '@/hooks/use-theme';
import { providerLabel } from '@/lib/session-presentation';
import { TEAM_NODE_HEIGHT, TEAM_NODE_WIDTH, teamGraphLayout } from '@/lib/team-graph';

export function TeamGraph({ members, agents, selectedId, onSelect, disabled = false }: {
  members: TeamMember[];
  agents: readonly AgentProfile[];
  selectedId?: string | null;
  onSelect: (id: string) => void;
  disabled?: boolean;
}) {
  const theme = useTheme();
  const { fontScale } = useWindowDimensions();
  const width = Math.ceil(TEAM_NODE_WIDTH * Math.max(1, fontScale));
  const height = Math.ceil(TEAM_NODE_HEIGHT * Math.max(1, fontScale));
  const layout = useMemo(() => teamGraphLayout(members, { width, height }), [members, width, height]);
  const profiles = useMemo(() => new Map(agents.map(a => [a.id, a])), [agents]);
  const parents = useMemo(() => new Map(members.map(m => [m.agentId, m.parentId])), [members]);
  if (layout.problem) return <Text accessibilityLiveRegion="polite" style={{ color: theme.warning }}>{layout.problem}</Text>;
  if (!layout.nodes.length) return null;
  return <ScrollView horizontal nestedScrollEnabled contentContainerStyle={styles.scroll}>
    <View style={{ width: layout.width, height: layout.height }}>
      {layout.lines.map((line, index) => <View key={`line-${index}`} pointerEvents="none" accessible={false}
        style={[styles.line, line, { backgroundColor: theme.borderStrong }]} />)}
      {layout.nodes.map(node => {
        const agent = profiles.get(node.id);
        const parent = parents.get(node.id);
        const reporting = parent ? `Reports to ${profiles.get(parent)?.name ?? 'unavailable agent'}` : 'Team lead';
        return <TeamGraphNode key={node.id} agent={agent} reporting={reporting} left={node.left} top={node.top} width={width} height={height}
          selected={selectedId === node.id} disabled={disabled} onPress={() => onSelect(node.id)} />;
      })}
    </View>
  </ScrollView>;
}

function TeamGraphNode({ agent, reporting, left, top, width, height, selected, disabled, onPress }: {
  agent?: AgentProfile; reporting: string; left: number; top: number; width: number; height: number;
  selected: boolean; disabled: boolean; onPress: () => void;
}) {
  const theme = useTheme();
  const [focused, setFocused] = useState(false);
  const name = agent?.name ?? 'Unavailable agent';
  const detail = agent ? `${providerLabel(agent.provider)} · ${agent.execution.environment === 'local' ? 'Local' : 'Nebius sandbox'}` : 'Refresh or remove this agent';
  return <Pressable accessibilityRole="button" accessibilityLabel={`${name}. ${reporting}. ${detail}`}
    accessibilityHint="Edit this agent’s reporting line" accessibilityState={{ selected, disabled }}
    focusable disabled={disabled} onFocus={() => setFocused(true)} onBlur={() => setFocused(false)} onPress={onPress}
    style={({ pressed }) => [styles.node, { left, top, width, height, backgroundColor: pressed ? theme.overlayStrong : theme.surface,
      borderColor: focused || selected ? theme.accent : theme.borderStrong, opacity: disabled ? .6 : 1 }]}>
    {agent && <ProviderIcon provider={agent.provider} size={22} />}
    <View style={styles.copy}>
      <Text numberOfLines={1} style={[styles.name, { color: theme.text }]}>{name}</Text>
      <Text numberOfLines={1} style={[styles.detail, { color: theme.textSecondary }]}>{detail}</Text>
      <Text numberOfLines={1} style={[styles.detail, { color: theme.textSecondary }]}>{reporting}</Text>
    </View>
  </Pressable>;
}

const styles = StyleSheet.create({
  scroll: { paddingVertical: 16 }, line: { position: 'absolute' },
  node: { position: 'absolute', borderWidth: 1,
    borderRadius: 24, paddingHorizontal: 14, flexDirection: 'row', alignItems: 'center', gap: 10 },
  copy: { flex: 1, minWidth: 0, gap: 2 }, name: { fontSize: 14, lineHeight: 19, fontWeight: '600' },
  detail: { fontSize: 12, lineHeight: 16 },
});
