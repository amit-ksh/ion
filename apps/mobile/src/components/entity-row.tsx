import type { ReactNode } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { MonoFont, Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

export type RowPosition = 'only' | 'first' | 'middle' | 'last';

/** Where row `index` of `count` sits in its bordered group. */
export function rowPosition(index: number, count: number): RowPosition {
  if (count <= 1) return 'only';
  if (index === 0) return 'first';
  return index === count - 1 ? 'last' : 'middle';
}

/**
 * A dense record row (Paperclip EntityRow). Rows of one group share a single
 * bordered, rounded container: each row draws its own slice of that border so
 * a virtualized list keeps one continuous frame with hairline dividers.
 */
export function EntityRow({
  position = 'only',
  leading,
  identifier,
  title,
  titleAccessory,
  subtitle,
  meta,
  trailing,
  footer,
  subtitleLines = 1,
  selected = false,
  onPress,
  onLongPress,
  accessibilityLabel,
  accessibilityHint,
}: {
  position?: RowPosition;
  leading?: ReactNode;
  identifier?: string | null;
  title: string;
  titleAccessory?: ReactNode;
  subtitle?: string | null;
  /** Mono metadata line under the subtitle (model, adapter, run id). */
  meta?: string | null;
  trailing?: ReactNode;
  /** Full-width content under the main line (instructions, progress). */
  footer?: ReactNode;
  /** Lines the subtitle may wrap to before it truncates. */
  subtitleLines?: number;
  selected?: boolean;
  onPress?: () => void;
  onLongPress?: () => void;
  accessibilityLabel?: string;
  accessibilityHint?: string;
}) {
  const theme = useTheme();
  const top = position === 'only' || position === 'first';
  const bottom = position === 'only' || position === 'last';
  const frame = [
    styles.frame,
    {
      backgroundColor: theme.ticket,
      borderColor: theme.ticketBorder,
      borderTopWidth: top ? 1 : 0,
      borderBottomWidth: bottom ? 1 : 0,
      borderTopLeftRadius: top ? Radius.medium : 0,
      borderTopRightRadius: top ? Radius.medium : 0,
      borderBottomLeftRadius: bottom ? Radius.medium : 0,
      borderBottomRightRadius: bottom ? Radius.medium : 0,
      marginBottom: bottom ? 12 : 0,
    },
  ];
  const label = accessibilityLabel ?? [title, identifier, subtitle, meta].filter(Boolean).join(', ');
  const main = (
    <View style={styles.main}>
      {leading ? <View style={styles.leading}>{leading}</View> : null}
      <View style={styles.copy}>
        <View style={styles.titleRow}>
          {identifier ? <Text style={[styles.identifier, { color: theme.textTertiary }]}>{identifier}</Text> : null}
          <Text numberOfLines={1} style={[styles.title, { color: theme.text }]}>{title}</Text>
          {titleAccessory}
        </View>
        {subtitle ? <Text numberOfLines={subtitleLines} style={[styles.subtitle, { color: theme.textSecondary }]}>{subtitle}</Text> : null}
        {meta ? <Text numberOfLines={1} style={[styles.meta, { color: theme.textTertiary }]}>{meta}</Text> : null}
      </View>
    </View>
  );
  return (
    <View style={frame}>
      {!top && <View style={[styles.divider, { backgroundColor: theme.border }]} />}
      <View collapsable={false} style={[styles.row, selected && { backgroundColor: theme.accentSoft }]}>
        {onPress ? (
          <Pressable
            accessibilityHint={accessibilityHint}
            accessibilityLabel={label}
            accessibilityRole="button"
            accessibilityState={{ selected }}
            onLongPress={onLongPress}
            onPress={onPress}
            style={({ pressed }) => [styles.press, pressed && { backgroundColor: theme.overlay }]}>
            {main}
          </Pressable>
        ) : (
          <View accessible accessibilityLabel={label} style={styles.press}>{main}</View>
        )}
        {trailing ? <View style={styles.trailing}>{trailing}</View> : null}
      </View>
      {footer ? <View style={styles.footer}>{footer}</View> : null}
    </View>
  );
}

const styles = StyleSheet.create({
  frame: { borderLeftWidth: 1, borderRightWidth: 1, overflow: 'hidden' },
  divider: { height: StyleSheet.hairlineWidth, marginLeft: 14 },
  row: { alignItems: 'center', flexDirection: 'row', minHeight: 60, paddingRight: 4 },
  press: { alignSelf: 'stretch', flex: 1, justifyContent: 'center', minHeight: 60, minWidth: 0, paddingLeft: 14, paddingRight: 8, paddingVertical: 10 },
  main: { alignItems: 'center', flexDirection: 'row', gap: 12 },
  leading: { alignItems: 'center', flexShrink: 0, justifyContent: 'center' },
  copy: { flex: 1, gap: 2, minWidth: 0 },
  titleRow: { alignItems: 'center', flexDirection: 'row', gap: 8, minWidth: 0 },
  identifier: { flexShrink: 0, fontFamily: MonoFont, fontSize: 12, fontVariant: ['tabular-nums'], lineHeight: 17 },
  title: { flexShrink: 1, fontSize: 15, fontWeight: '500', lineHeight: 20 },
  subtitle: { fontSize: 13, lineHeight: 18 },
  meta: { fontFamily: MonoFont, fontSize: 11.5, lineHeight: 16 },
  trailing: { alignItems: 'center', flexDirection: 'row', flexShrink: 0, gap: 4 },
  footer: { gap: 10, paddingBottom: 14, paddingHorizontal: 14 },
});
