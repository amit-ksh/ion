import type { AgentProfile, TeamMember } from '@waku/client';
import { useMemo, useRef, useState } from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View, useWindowDimensions } from 'react-native';
import Svg, { Path } from 'react-native-svg';

import { AgentAvatar, type AgentPresence } from './agent-avatar';
import { MonoFont, Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { providerLabel } from '@/lib/session-presentation';
import { teamGraphLayout } from '@/lib/team-graph';

/** Paperclip org chart geometry: 200dp cards, 1.5dp elbow connectors. */
const NODE_WIDTH = 200;
const NODE_HEIGHT = 84;
const PAD = 20;

export function agentAdapterLabel(agent: AgentProfile): string {
  return `${providerLabel(agent.provider)} · ${agent.execution.environment === 'local' ? 'Local' : 'Nebius'}`;
}

/**
 * A team as an org chart: the lead on top, reports below, joined by
 * orthogonal elbow connectors (parent bottom-center, down to the midline,
 * across, down to the child top-center) on a muted canvas. Wider than the
 * phone, the chart pans horizontally and opens centered on the lead.
 */
export function TeamGraph({ members, agents, selectedId, onSelect, disabled = false, presence }: {
  members: TeamMember[];
  agents: readonly AgentProfile[];
  selectedId?: string | null;
  onSelect: (id: string) => void;
  disabled?: boolean;
  presence?: ReadonlyMap<string, AgentPresence>;
}) {
  const theme = useTheme();
  const { fontScale } = useWindowDimensions();
  const width = Math.ceil(NODE_WIDTH * Math.max(1, fontScale));
  const height = Math.ceil(NODE_HEIGHT * Math.max(1, fontScale));
  const layout = useMemo(() => teamGraphLayout(members, { width, height }), [members, width, height]);
  const profiles = useMemo(() => new Map(agents.map(a => [a.id, a])), [agents]);
  const parents = useMemo(() => new Map(members.map(m => [m.agentId, m.parentId])), [members]);
  const scroll = useRef<ScrollView>(null);
  const [viewport, setViewport] = useState(0);
  const measured = useRef({ viewport: 0, content: 0, centered: false });
  // Open centered on the lead once both widths are known; later pans stay put.
  const center = () => {
    const m = measured.current;
    if (m.centered || !m.viewport || !m.content) return;
    m.centered = true;
    if (m.content > m.viewport) scroll.current?.scrollTo({ x: (m.content - m.viewport) / 2, animated: false });
  };
  const edges = useMemo(() => {
    const positions = new Map(layout.nodes.map(n => [n.id, n]));
    return layout.nodes.flatMap(child => {
      const parentId = parents.get(child.id);
      const parent = parentId ? positions.get(parentId) : undefined;
      if (!parent) return [];
      const x1 = parent.left + width / 2;
      const y1 = parent.top + height;
      const x2 = child.left + width / 2;
      const y2 = child.top;
      const mid = (y1 + y2) / 2;
      return [{ id: `${parent.id}-${child.id}`, d: `M ${x1} ${y1} L ${x1} ${mid} L ${x2} ${mid} L ${x2} ${y2}` }];
    });
  }, [layout.nodes, parents, width, height]);
  if (layout.problem) return <Text accessibilityLiveRegion="polite" style={{ color: theme.warning }}>{layout.problem}</Text>;
  if (!layout.nodes.length) return null;
  const content = layout.width + PAD * 2;
  return (
    <View
      onLayout={event => { measured.current.viewport = event.nativeEvent.layout.width; setViewport(event.nativeEvent.layout.width); center(); }}
      style={[styles.canvas, { backgroundColor: theme.inset, borderColor: theme.border }]}>
      <ScrollView
        ref={scroll}
        horizontal
        nestedScrollEnabled
        showsHorizontalScrollIndicator={false}
        contentContainerStyle={{ minWidth: viewport, alignItems: 'center', padding: PAD }}
        onContentSizeChange={contentWidth => { measured.current.content = contentWidth; center(); }}>
        <View style={{ width: layout.width, height: layout.height }}>
          <Svg pointerEvents="none" width={layout.width} height={layout.height} style={StyleSheet.absoluteFill}>
            {edges.map(edge => <Path key={edge.id} d={edge.d} fill="none" stroke={theme.borderStrong} strokeWidth={1.5} />)}
          </Svg>
          {layout.nodes.map(node => {
            const agent = profiles.get(node.id);
            const parent = parents.get(node.id);
            const reporting = parent ? `Reports to ${profiles.get(parent)?.name ?? 'unavailable agent'}` : 'Team lead';
            return <OrgNode key={node.id} agent={agent} lead={!parent} reporting={reporting} left={node.left} top={node.top}
              width={width} height={height} selected={selectedId === node.id} disabled={disabled}
              presence={agent ? presence?.get(node.id) ?? 'idle' : 'missing'} onPress={() => onSelect(node.id)} />;
          })}
        </View>
      </ScrollView>
      {content > viewport && viewport > 0 && (
        <Text style={[styles.hint, { color: theme.textTertiary }]}>Swipe to pan the chart</Text>
      )}
    </View>
  );
}

function OrgNode({ agent, lead, reporting, left, top, width, height, selected, disabled, presence, onPress }: {
  agent?: AgentProfile; lead: boolean; reporting: string; left: number; top: number; width: number; height: number;
  selected: boolean; disabled: boolean; presence: AgentPresence; onPress: () => void;
}) {
  const theme = useTheme();
  const [focused, setFocused] = useState(false);
  const name = agent?.name ?? 'Unavailable agent';
  const role = agent ? agent.expertise || 'Agent' : 'Refresh or remove this agent';
  const adapter = agent ? agentAdapterLabel(agent) : null;
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={[name, lead ? 'Lead' : null, role, adapter, reporting].filter(Boolean).join('. ')}
      accessibilityHint="Edit this agent's reporting line"
      accessibilityState={{ selected, disabled }}
      focusable
      disabled={disabled}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      onPress={onPress}
      style={({ pressed }) => [styles.node, {
        left, top, width, height,
        backgroundColor: pressed ? theme.surfaceMuted : theme.ticket,
        borderColor: focused || selected ? theme.accent : theme.ticketBorder,
        borderWidth: focused || selected ? 1.5 : 1,
        boxShadow: theme.elevationRest,
        opacity: disabled ? 0.6 : 1,
      }]}>
      <AgentAvatar provider={agent?.provider} presence={presence} ring={theme.ticket} />
      <View style={styles.copy}>
        <View style={styles.nameRow}>
          <Text numberOfLines={1} style={[styles.name, { color: theme.text }]}>{name}</Text>
          {lead && (
            <View style={[styles.lead, { backgroundColor: theme.primary }]}>
              <Text style={[styles.leadLabel, { color: theme.onPrimary }]}>Lead</Text>
            </View>
          )}
        </View>
        <Text numberOfLines={1} style={[styles.role, { color: theme.textSecondary }]}>{role}</Text>
        {adapter && <Text numberOfLines={1} style={[styles.adapter, { color: theme.textTertiary }]}>{adapter}</Text>}
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  canvas: { borderRadius: Radius.medium, borderWidth: StyleSheet.hairlineWidth, overflow: 'hidden' },
  hint: { fontSize: 12, lineHeight: 16, paddingBottom: 10, textAlign: 'center' },
  node: {
    alignItems: 'center', borderRadius: Radius.medium, flexDirection: 'row', gap: 12,
    paddingHorizontal: 14, paddingVertical: 12, position: 'absolute',
  },
  copy: { flex: 1, gap: 2, minWidth: 0 },
  nameRow: { alignItems: 'center', flexDirection: 'row', gap: 6 },
  name: { flexShrink: 1, fontSize: 14, fontWeight: '600', lineHeight: 19 },
  lead: { borderRadius: Radius.pill, justifyContent: 'center', minHeight: 18, paddingHorizontal: 7 },
  leadLabel: { fontSize: 11, fontWeight: '700', lineHeight: 14 },
  role: { fontSize: 12, lineHeight: 16 },
  adapter: { fontFamily: MonoFont, fontSize: 11, lineHeight: 15 },
});
