import type { ReactNode } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import Svg, { Line } from 'react-native-svg';

import { AppSymbol } from './app-symbol';
import { PressableScale } from './press-scale';
import { MonoFont, Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { TICKET_LABELS, type TicketState } from '@/lib/ticket';

type SymbolName = Parameters<typeof AppSymbol>[0]['name'];
export type PillTone = 'accent' | 'neutral' | 'success' | 'danger' | 'warning';
export type TicketStatus = { label: string; icon: SymbolName; tone: PillTone };

const STATE_ICONS: Record<TicketState, SymbolName> = {
  needsYou: { android: 'error', web: 'error' },
  working: { android: 'progress_activity', web: 'progress_activity' },
  background: { android: 'hourglass_empty', web: 'hourglass_empty' },
  failed: { android: 'close', web: 'close' },
  ready: { android: 'check', web: 'check' },
};
const STATE_TONES: Record<TicketState, PillTone> = {
  needsYou: 'accent', working: 'accent', background: 'neutral', failed: 'danger', ready: 'success',
};

export function ticketStatus(state: TicketState): TicketStatus {
  return { label: TICKET_LABELS[state], icon: STATE_ICONS[state], tone: STATE_TONES[state] };
}

/** Icon + label pill; the label always carries the meaning, the hue only echoes it. */
export function StatusPill({ label, icon, tone }: TicketStatus) {
  const theme = useTheme();
  const [color, fill] = {
    accent: [theme.accent, theme.accentSoft],
    neutral: [theme.textSecondary, theme.overlayStrong],
    success: [theme.success, theme.successSoft],
    danger: [theme.danger, theme.dangerSoft],
    warning: [theme.warning, theme.warningSoft],
  }[tone];
  return (
    <View style={[styles.pill, { backgroundColor: fill }]}>
      <AppSymbol name={icon} size={13} tintColor={color} />
      <Text numberOfLines={1} style={[styles.pillLabel, { color }]}>{label}</Text>
    </View>
  );
}

/** The tear line between a ticket's stub and its body. */
export function Perforation() {
  const theme = useTheme();
  return (
    <View style={styles.perforation}>
      <Svg height={1} width="100%">
        <Line x1="0" y1="0.5" x2="100%" y2="0.5" stroke={theme.borderStrong} strokeWidth={1} strokeDasharray="3 3" />
      </Svg>
    </View>
  );
}

/**
 * A record on the rail: a stub with its mono order number and time, a tear
 * line, then the title, its station (mark + place) and a status pill. Work
 * waiting on the user holds a lime edge and stub. Interactive children (team
 * graphs, actions) sit below the pressable body so each stays reachable.
 */
export function TicketCard({
  number,
  time,
  title,
  mark,
  station,
  status,
  needsYou = false,
  selected = false,
  onPress,
  onLongPress,
  accessibilityLabel,
  accessibilityHint,
  titleLines = 2,
  children,
}: {
  number: string;
  time?: string | null;
  title: string;
  mark?: ReactNode;
  station?: string | null;
  status?: TicketStatus | null;
  needsYou?: boolean;
  selected?: boolean;
  onPress?: () => void;
  onLongPress?: () => void;
  accessibilityLabel?: string;
  accessibilityHint?: string;
  titleLines?: number;
  children?: ReactNode;
}) {
  const theme = useTheme();
  const edge = needsYou ? theme.primary : selected ? theme.accent : theme.ticketBorder;
  const content = (pressed: boolean) => (
    <>
      <View collapsable={false} style={[styles.stub, needsYou && { backgroundColor: theme.accentSoft }, pressed && { backgroundColor: theme.overlay }]}>
        <Text style={[styles.mono, { color: needsYou ? theme.accent : theme.textTertiary }]}>{number}</Text>
        <View style={styles.grow} />
        {time ? <Text style={[styles.mono, { color: theme.textTertiary }]}>{time}</Text> : null}
      </View>
      <Perforation />
      <View collapsable={false} style={[styles.body, pressed && { backgroundColor: theme.overlay }]}>
        <Text numberOfLines={titleLines} style={[styles.title, { color: theme.text }]}>{title}</Text>
        {(mark || station || status) && (
          <View style={styles.meta}>
            {mark}
            <Text numberOfLines={1} style={[styles.station, { color: theme.textSecondary }]}>{station ?? ''}</Text>
            {status && <StatusPill {...status} />}
          </View>
        )}
      </View>
    </>
  );
  const label = accessibilityLabel ?? [title, station, status?.label, time].filter(Boolean).join(', ');
  const card = [styles.card, { backgroundColor: theme.ticket, borderColor: edge, boxShadow: theme.elevationRest }];
  if (!onPress) return <View accessible={!children} accessibilityLabel={label} style={card}>{content(false)}{children ? <View style={styles.extra}>{children}</View> : null}</View>;
  if (children) {
    return (
      <View style={card}>
        <Pressable accessibilityRole="button" accessibilityLabel={label} accessibilityHint={accessibilityHint}
          accessibilityState={{ selected }} onPress={onPress} onLongPress={onLongPress}>
          {({ pressed }) => content(pressed)}
        </Pressable>
        <View style={styles.extra}>{children}</View>
      </View>
    );
  }
  return (
    <PressableScale accessibilityRole="button" accessibilityLabel={label} accessibilityHint={accessibilityHint}
      accessibilityState={{ selected }} onPress={onPress} onLongPress={onLongPress} style={card}>
      {content(false)}
    </PressableScale>
  );
}

const styles = StyleSheet.create({
  card: { borderRadius: Radius.medium, borderWidth: 1 },
  stub: {
    alignItems: 'center',
    borderTopLeftRadius: Radius.medium - 1,
    borderTopRightRadius: Radius.medium - 1,
    flexDirection: 'row',
    gap: 8,
    minHeight: 34,
    paddingHorizontal: 14,
    paddingVertical: 8,
  },
  mono: { fontFamily: MonoFont, fontSize: 12, lineHeight: 16, fontVariant: ['tabular-nums'] },
  grow: { flex: 1 },
  perforation: { height: 1, marginHorizontal: 14 },
  body: {
    borderBottomLeftRadius: Radius.medium - 1,
    borderBottomRightRadius: Radius.medium - 1,
    gap: 10,
    paddingBottom: 14,
    paddingHorizontal: 14,
    paddingTop: 10,
  },
  title: { fontSize: 15.5, fontWeight: '500', lineHeight: 21 },
  meta: { alignItems: 'center', flexDirection: 'row', gap: 8, minHeight: 24 },
  station: { flex: 1, fontSize: 13, lineHeight: 18 },
  pill: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    flexDirection: 'row',
    flexShrink: 0,
    gap: 4,
    maxWidth: 170,
    minHeight: 24,
    paddingHorizontal: 9,
  },
  pillLabel: { fontSize: 12, fontWeight: '600', lineHeight: 16 },
  extra: { gap: 10, paddingBottom: 14, paddingHorizontal: 14 },
});
