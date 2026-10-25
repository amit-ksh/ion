import type { AgentProfile, Team } from '@waku/client';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Pressable, StyleSheet, Text, View, useWindowDimensions } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, { Easing, useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';
import Svg, { Circle, Defs, Pattern, Rect } from 'react-native-svg';
import { scheduleOnRN } from 'react-native-worklets';

import { AgentAvatar, type AgentPresence } from './agent-avatar';
import { AppSymbol } from './app-symbol';
import { agentAdapterLabel } from './team-graph';
import { MonoFont, Radius } from '@/constants/theme';
import { useReducedMotion } from '@/hooks/use-reduced-motion';
import { useTheme } from '@/hooks/use-theme';
import { ORG_NODE, orgChartLayout, type OrgCluster, type OrgNode } from '@/lib/org-chart';

type SymbolName = Parameters<typeof AppSymbol>[0]['name'];
export type TeamAction = 'task' | 'runs' | 'edit';

const MIN_ZOOM = 0.35;
const MAX_ZOOM = 2;
const GRID = 24;

function clamp(value: number) {
  'worklet';
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value));
}

/**
 * Every agent and its reporting lines on one canvas that pans in all
 * directions and pinch-zooms. Teams are clusters under a header chip; each
 * agent card carries a "+" on its bottom edge for subagents and membership.
 */
export function OrgCanvas({ teams, agents, presence, onOpenAgent, onAgentMenu, onTeamAction, onNewTeam }: {
  teams: readonly Team[];
  agents: readonly AgentProfile[];
  presence: (agentId: string) => AgentPresence;
  onOpenAgent: (agentId: string) => void;
  onAgentMenu: (node: OrgNode) => void;
  onTeamAction: (team: Team, action: TeamAction) => void;
  onNewTeam: () => void;
}) {
  const theme = useTheme();
  const reduced = useReducedMotion();
  const { fontScale } = useWindowDimensions();
  const scale = Math.max(1, fontScale);
  const layout = useMemo(() => orgChartLayout(teams, agents, scale), [teams, agents, scale]);
  const profiles = useMemo(() => new Map(agents.map(agent => [agent.id, agent])), [agents]);
  const node = { width: Math.ceil(ORG_NODE.width * scale), height: Math.ceil(ORG_NODE.height * scale) };
  const tx = useSharedValue(0);
  const ty = useSharedValue(0);
  const zoom = useSharedValue(1);
  const startX = useSharedValue(0);
  const startY = useSharedValue(0);
  const startZoom = useSharedValue(1);
  const focalX = useSharedValue(0);
  const focalY = useSharedValue(0);
  const [percent, setPercent] = useState(100);
  const viewport = useRef({ width: 0, height: 0, placed: false });
  const busyUntil = useRef(0);
  const healTimers = useRef<ReturnType<typeof setTimeout>[]>([]);

  // Reanimated copies a settled transform back into React one to two seconds
  // after it stops changing. When the JS thread stalls through that window the
  // copy is skipped, and the next React commit snaps the canvas back to an
  // older view. Re-asserting the current transform after each render and
  // after each move settles puts it back and gives the copy another chance.
  function reassert() {
    if (Date.now() < busyUntil.current) return;
    tx.modify();
    ty.modify();
    zoom.modify();
  }
  function settle() {
    healTimers.current.forEach(clearTimeout);
    healTimers.current = [1500, 3000].map(delay => setTimeout(reassert, delay));
  }
  useEffect(() => () => healTimers.current.forEach(clearTimeout), []);
  useEffect(reassert);

  function move(next: { x: number; y: number; zoom: number }) {
    const timing = { duration: 220, easing: Easing.bezier(0.25, 1, 0.5, 1) };
    busyUntil.current = reduced ? 0 : Date.now() + 320;
    settle();
    tx.value = reduced ? next.x : withTiming(next.x, timing);
    ty.value = reduced ? next.y : withTiming(next.y, timing);
    zoom.value = reduced ? next.zoom : withTiming(next.zoom, timing);
    setPercent(Math.round(next.zoom * 100));
  }
  function fit() {
    const { width, height } = viewport.current;
    if (!width || !layout.width) return;
    const next = clamp(Math.min((width - 24) / layout.width, (height - 24) / layout.height, 1));
    move({ zoom: next, x: (width - layout.width * next) / 2, y: Math.max(12, (height - layout.height * next) / 2) });
  }
  /** Open on the first cluster, readable, its chip centered at the top. */
  function home() {
    const { width } = viewport.current;
    const first = layout.clusters[0];
    if (!width || !first) return;
    const next = clamp(Math.min(1, (width - 32) / first.width));
    tx.value = width / 2 - (first.left + first.width / 2) * next;
    ty.value = 16 - first.top * next;
    zoom.value = next;
    setPercent(Math.round(next * 100));
    settle();
  }
  function step(factor: number) {
    const { width, height } = viewport.current;
    const current = zoom.value;
    const next = clamp(current * factor);
    const k = next / current;
    move({ zoom: next, x: width / 2 - (width / 2 - tx.value) * k, y: height / 2 - (height / 2 - ty.value) * k });
  }

  // Agents can arrive after the canvas is measured; place the view once both exist.
  useEffect(() => {
    if (viewport.current.placed || !viewport.current.width || !layout.clusters.length) return;
    viewport.current.placed = true;
    home();
  });

  const pan = Gesture.Pan().maxPointers(1).minDistance(6)
    .onStart(() => { startX.value = tx.value; startY.value = ty.value; })
    .onUpdate(event => { tx.value = startX.value + event.translationX; ty.value = startY.value + event.translationY; })
    .onEnd(() => { scheduleOnRN(settle); });
  const pinch = Gesture.Pinch()
    .onStart(event => {
      startZoom.value = zoom.value; startX.value = tx.value; startY.value = ty.value;
      focalX.value = event.focalX; focalY.value = event.focalY;
    })
    .onUpdate(event => {
      const next = clamp(startZoom.value * event.scale);
      const k = next / startZoom.value;
      zoom.value = next;
      tx.value = event.focalX - (focalX.value - startX.value) * k;
      ty.value = event.focalY - (focalY.value - startY.value) * k;
    })
    .onEnd(() => { scheduleOnRN(setPercent, Math.round(zoom.value * 100)); scheduleOnRN(settle); });
  const gesture = Gesture.Simultaneous(pan, pinch);
  const contentStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: tx.value }, { translateY: ty.value }, { scale: zoom.value }],
  }));

  return (
    <View style={styles.frame}>
      <View
        onLayout={event => {
          viewport.current.width = event.nativeEvent.layout.width;
          viewport.current.height = event.nativeEvent.layout.height;
          if (!viewport.current.placed && layout.clusters.length) { viewport.current.placed = true; home(); }
        }}
        style={[styles.viewport, { backgroundColor: theme.inset, borderColor: theme.border }]}>
        <Svg pointerEvents="none" width="100%" height="100%" style={StyleSheet.absoluteFill}>
          <Defs>
            <Pattern id="org-dots" width={GRID} height={GRID} patternUnits="userSpaceOnUse">
              <Circle cx={GRID / 2} cy={GRID / 2} r={1.1} fill={theme.borderStrong} />
            </Pattern>
          </Defs>
          <Rect width="100%" height="100%" fill="url(#org-dots)" />
        </Svg>
        <GestureDetector gesture={gesture}>
          <View style={StyleSheet.absoluteFill} collapsable={false}>
            <Animated.View style={[styles.content, { width: layout.width, height: layout.height }, contentStyle]}>
              {layout.edges.flatMap(edge => edge.segments.map((segment, index) => (
                <View key={`${edge.key}:${index}`} pointerEvents="none" style={[styles.segment, segment, { backgroundColor: theme.borderStrong }]} />
              )))}
              {layout.clusters.map(cluster => (
                <ClusterChip key={cluster.key} cluster={cluster} onTeamAction={onTeamAction} onNewTeam={onNewTeam} />
              ))}
              {layout.clusters.filter(cluster => cluster.problem).map(cluster => (
                <Text key={`${cluster.key}:problem`} style={[styles.problem, { color: theme.warning, left: cluster.chip.left, top: cluster.chip.top + cluster.chip.height + 12, width: cluster.chip.width }]}>
                  {cluster.problem}
                </Text>
              ))}
              {layout.nodes.map(item => (
                <AgentCard key={item.key} node={item} agent={profiles.get(item.agentId)} width={node.width} height={node.height}
                  presence={profiles.has(item.agentId) ? presence(item.agentId) : 'missing'}
                  onOpen={() => onOpenAgent(item.agentId)} onMenu={() => onAgentMenu(item)} />
              ))}
            </Animated.View>
          </View>
        </GestureDetector>
      </View>
      {/* A footer, not an overlay: floating controls covered cards' buttons and badges. */}
      <View style={[styles.footer, { backgroundColor: theme.background, borderTopColor: theme.border }]}>
        <View style={styles.legend}>
          <Text style={[styles.legendText, { color: theme.textSecondary }]}>Drag to move</Text>
          {/* Each dot stays with its label when the legend wraps. */}
          <View style={styles.legendItem}>
            <View style={[styles.legendDot, { backgroundColor: theme.success }]} />
            <Text style={[styles.legendText, { color: theme.textSecondary }]}>Ready</Text>
          </View>
          <View style={styles.legendItem}>
            <View style={[styles.legendDot, { backgroundColor: theme.warning }]} />
            <Text style={[styles.legendText, { color: theme.textSecondary }]}>Harness missing</Text>
          </View>
        </View>
        <View style={[styles.controlRow, { backgroundColor: theme.ticket, borderColor: theme.ticketBorder }]}>
          <ControlButton icon={{ ios: 'minus', android: 'remove', web: 'remove' }} label="Zoom out" onPress={() => step(0.8)} />
          <Text accessibilityLabel={`Zoom ${percent} percent`} style={[styles.mono, styles.zoomLabel, { color: theme.textSecondary }]}>{percent}%</Text>
          <ControlButton icon={{ ios: 'plus', android: 'add', web: 'add' }} label="Zoom in" onPress={() => step(1.25)} />
          <View style={[styles.controlDivider, { backgroundColor: theme.border }]} />
          <ControlButton icon={{ ios: 'arrow.up.left.and.arrow.down.right', android: 'fit_screen', web: 'fit_screen' }} label="Fit chart to screen" onPress={fit} />
        </View>
      </View>
    </View>
  );
}

function ControlButton({ icon, label, onPress }: { icon: SymbolName; label: string; onPress: () => void }) {
  const theme = useTheme();
  return (
    <Pressable accessibilityRole="button" accessibilityLabel={label} onPress={onPress}
      style={({ pressed }) => [styles.control, pressed && { backgroundColor: theme.overlayStrong }]}>
      <AppSymbol name={icon} size={20} tintColor={theme.text} />
    </Pressable>
  );
}

/** A team's header chip: icon tile, name, mono roster size and small actions. */
function ClusterChip({ cluster, onTeamAction, onNewTeam }: {
  cluster: OrgCluster;
  onTeamAction: (team: Team, action: TeamAction) => void;
  onNewTeam: () => void;
}) {
  const theme = useTheme();
  const team = cluster.team;
  const reports = Math.max(0, cluster.count - 1);
  const count = team ? cluster.count === 1 ? 'Lead only' : `Lead + ${reports} ${reports === 1 ? 'agent' : 'agents'}`
    : `${cluster.count} ${cluster.count === 1 ? 'agent' : 'agents'}`;
  const name = team?.name ?? 'Not on a team';
  return (
    <View accessibilityLabel={`${name}, ${count}`} style={[styles.chip, {
      left: cluster.chip.left, top: cluster.chip.top, width: cluster.chip.width, minHeight: cluster.chip.height,
      backgroundColor: theme.ticket, borderColor: theme.ticketBorder, boxShadow: theme.elevationRest,
    }]}>
      <View style={styles.chipHead}>
        <View style={[styles.chipTile, { backgroundColor: team ? theme.accentSoft : theme.surfaceMuted }]}>
          <AppSymbol name={team ? { android: 'groups', web: 'groups' } : { android: 'person_search', web: 'person_search' }} size={18}
            tintColor={team ? theme.accent : theme.textSecondary} />
        </View>
        <View style={styles.chipCopy}>
          <Text accessibilityRole="header" numberOfLines={1} style={[styles.chipName, { color: theme.text }]}>{name}</Text>
          <Text numberOfLines={1} style={[styles.mono, { color: theme.textTertiary }]}>{count}</Text>
        </View>
      </View>
      <View style={styles.chipActions}>
        {team ? <>
          <ChipAction icon={{ android: 'add_task', web: 'add_task' }} label="New task" accessibilityLabel={`New task for ${name}`} onPress={() => onTeamAction(team, 'task')} />
          <ChipAction icon={{ android: 'history', web: 'history' }} label="Runs" accessibilityLabel={`Runs of ${name}`} onPress={() => onTeamAction(team, 'runs')} />
          <ChipAction icon={{ android: 'edit', web: 'edit' }} label="Edit" accessibilityLabel={`Edit ${name}`} onPress={() => onTeamAction(team, 'edit')} />
        </> : <ChipAction icon={{ android: 'group_add', web: 'group_add' }} label="New team" onPress={onNewTeam} />}
      </View>
    </View>
  );
}

function ChipAction({ icon, label, accessibilityLabel, onPress }: { icon: SymbolName; label: string; accessibilityLabel?: string; onPress: () => void }) {
  const theme = useTheme();
  return (
    <Pressable accessibilityRole="button" accessibilityLabel={accessibilityLabel ?? label} onPress={onPress} style={styles.chipAction}>
      {({ pressed }) => (
        <View collapsable={false} style={[styles.chipActionPill, { backgroundColor: pressed ? theme.overlayStrong : theme.overlay, borderColor: theme.border }]}>
          <AppSymbol name={icon} size={14} tintColor={theme.textSecondary} />
          <Text style={[styles.chipActionLabel, { color: theme.text }]}>{label}</Text>
        </View>
      )}
    </Pressable>
  );
}

/** An agent on the canvas (Paperclip org card) with its "+" on the bottom edge. */
function AgentCard({ node, agent, width, height, presence, onOpen, onMenu }: {
  node: OrgNode; agent?: AgentProfile; width: number; height: number; presence: AgentPresence;
  onOpen: () => void; onMenu: () => void;
}) {
  const theme = useTheme();
  const name = agent?.name ?? 'Unavailable agent';
  const role = agent ? agent.expertise || 'Agent' : 'Refresh or remove this agent';
  const adapter = agent ? agentAdapterLabel(agent) : null;
  const state = presence === 'missing' ? 'Harness missing' : 'Ready';
  return <>
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={[name, node.lead ? 'Lead' : null, role, adapter, state].filter(Boolean).join('. ')}
      accessibilityHint="Opens this agent"
      onPress={onOpen}
      style={({ pressed }) => [styles.card, {
        left: node.left, top: node.top, width, height,
        backgroundColor: pressed ? theme.surfaceMuted : theme.ticket,
        borderColor: node.lead ? theme.primary : theme.ticketBorder,
        boxShadow: theme.elevationRest,
      }]}>
      <AgentAvatar provider={agent?.provider} presence={presence} ring={theme.ticket} />
      <View style={styles.cardCopy}>
        <Text numberOfLines={1} style={[styles.cardName, { color: theme.text }]}>{name}</Text>
        <Text numberOfLines={1} style={[styles.cardRole, { color: theme.textSecondary }]}>{role}</Text>
        {adapter && <Text numberOfLines={1} style={[styles.cardAdapter, { color: theme.textTertiary }]}>{adapter}</Text>}
      </View>
      {node.lead && (
        <View style={[styles.lead, { backgroundColor: theme.primary }]}>
          <Text style={[styles.leadLabel, { color: theme.onPrimary }]}>Lead</Text>
        </View>
      )}
    </Pressable>
    <Pressable accessibilityRole="button" accessibilityLabel={`Add under ${name}`} accessibilityHint="New subagent, add an existing agent, or edit"
      onPress={onMenu} style={[styles.plusTarget, { left: node.left + width / 2 - 24, top: node.top + height - 24 }]}>
      {({ pressed }) => (
        <View collapsable={false} style={[styles.plus, {
          backgroundColor: pressed ? theme.primary : theme.ticket, borderColor: pressed ? theme.primary : theme.borderStrong, boxShadow: theme.elevationRest,
        }]}>
          <AppSymbol name={{ ios: 'plus', android: 'add', web: 'add' }} size={16} tintColor={pressed ? theme.onPrimary : theme.textSecondary} />
        </View>
      )}
    </Pressable>
  </>;
}

const styles = StyleSheet.create({
  viewport: { borderTopWidth: StyleSheet.hairlineWidth, flex: 1, overflow: 'hidden' },
  content: { left: 0, position: 'absolute', top: 0, transformOrigin: 'left top' },
  segment: { position: 'absolute' },
  problem: { fontSize: 13, lineHeight: 18, position: 'absolute', textAlign: 'center' },
  frame: { flex: 1 },
  footer: {
    alignItems: 'center', borderTopWidth: StyleSheet.hairlineWidth, flexDirection: 'row', gap: 12,
    paddingHorizontal: 12, paddingVertical: 8,
  },
  controlRow: { alignItems: 'center', borderRadius: Radius.pill, borderWidth: 1, flexDirection: 'row', overflow: 'hidden' },
  control: { alignItems: 'center', height: 44, justifyContent: 'center', width: 44 },
  controlDivider: { height: 24, width: StyleSheet.hairlineWidth },
  zoomLabel: { minWidth: 40, textAlign: 'center' },
  mono: { fontFamily: MonoFont, fontSize: 12, fontVariant: ['tabular-nums'], lineHeight: 16 },
  legend: { alignItems: 'center', columnGap: 12, flex: 1, flexDirection: 'row', flexWrap: 'wrap', rowGap: 2 },
  legendItem: { alignItems: 'center', flexDirection: 'row', gap: 6 },
  legendText: { fontSize: 12, lineHeight: 16 },
  legendDot: { borderRadius: 4, height: 8, width: 8 },
  chip: { borderRadius: 14, borderWidth: 1, gap: 2, paddingBottom: 8, paddingHorizontal: 12, paddingTop: 10, position: 'absolute' },
  chipHead: { alignItems: 'center', flexDirection: 'row', gap: 10 },
  chipTile: { alignItems: 'center', borderRadius: 10, height: 36, justifyContent: 'center', width: 36 },
  chipCopy: { flex: 1, gap: 1, minWidth: 0 },
  chipName: { fontSize: 15, fontWeight: '600', lineHeight: 20 },
  chipActions: { flexDirection: 'row', flexWrap: 'wrap', gap: 6 },
  chipAction: { justifyContent: 'center', minHeight: 48 },
  chipActionPill: {
    alignItems: 'center', borderRadius: Radius.pill, borderWidth: StyleSheet.hairlineWidth, flexDirection: 'row', gap: 4,
    minHeight: 32, paddingHorizontal: 10,
  },
  chipActionLabel: { fontSize: 12.5, fontWeight: '500', lineHeight: 16 },
  card: {
    alignItems: 'center', borderRadius: Radius.medium, borderWidth: 1, flexDirection: 'row', gap: 12,
    paddingHorizontal: 14, paddingVertical: 12, position: 'absolute',
  },
  cardCopy: { flex: 1, gap: 2, minWidth: 0 },
  cardName: { fontSize: 14, fontWeight: '600', lineHeight: 19 },
  cardRole: { fontSize: 12, lineHeight: 16 },
  cardAdapter: { fontFamily: MonoFont, fontSize: 11, lineHeight: 15 },
  lead: { borderRadius: Radius.pill, justifyContent: 'center', minHeight: 18, paddingHorizontal: 8, position: 'absolute', right: 12, top: -10 },
  leadLabel: { fontSize: 11, fontWeight: '700', lineHeight: 14 },
  plusTarget: { alignItems: 'center', height: 48, justifyContent: 'center', position: 'absolute', width: 48 },
  plus: { alignItems: 'center', borderRadius: 14, borderWidth: 1, height: 28, justifyContent: 'center', width: 28 },
});
