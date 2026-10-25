import { useEffect, useRef } from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';

import { Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

/** Pill track of tabs; the selected one lifts onto a white ticket segment. */
export function SegmentedTabs<T extends string>({ tabs, value, onChange, counts }: {
  tabs: readonly T[];
  value: T;
  onChange: (tab: T) => void;
  /** Optional tabular badge per tab (e.g. waiting decisions). */
  counts?: Partial<Record<T, number>>;
}) {
  const theme = useTheme();
  const scroller = useRef<ScrollView>(null);
  const frames = useRef(new Map<T, { x: number; width: number }>());
  const viewport = useRef(0);
  // At large font sizes the track can outgrow the screen and scroll; keep the
  // selected tab fully in view instead of leaving it cut at the edge.
  function reveal(tab: T) {
    const frame = frames.current.get(tab);
    if (!frame || !viewport.current) return;
    if (tab === tabs[tabs.length - 1]) scroller.current?.scrollToEnd({ animated: false });
    else scroller.current?.scrollTo({ x: Math.max(0, frame.x + frame.width + 16 - viewport.current), animated: false });
  }
  useEffect(() => reveal(value), [value]);
  return (
    <ScrollView ref={scroller} horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.scroll}
      onLayout={event => { viewport.current = event.nativeEvent.layout.width; reveal(value); }}>
      <View accessibilityRole="tablist" style={[styles.track, { backgroundColor: theme.overlay, borderColor: theme.border }]}>
        {tabs.map(tab => {
          const selected = value === tab;
          const count = counts?.[tab] ?? 0;
          return (
            <Pressable key={tab} accessibilityRole="tab" accessibilityState={{ selected }}
              accessibilityLabel={count ? `${tab}, ${count} waiting` : tab}
              onPress={() => onChange(tab)} style={styles.tab}
              onLayout={event => {
                const { x, width } = event.nativeEvent.layout;
                frames.current.set(tab, { x, width });
                if (tab === value) reveal(tab);
              }}>
              {({ pressed }) => (
                <View collapsable={false} style={[styles.segment, selected
                  ? { backgroundColor: theme.ticket, boxShadow: theme.elevationRest }
                  : { backgroundColor: pressed ? theme.overlayStrong : 'transparent' }]}>
                  <Text style={[styles.label, { color: selected ? theme.text : theme.textSecondary, fontWeight: selected ? '600' : '500' }]}>{tab}</Text>
                  {count > 0 && (
                    <View style={[styles.badge, { backgroundColor: theme.primary }]}>
                      <Text style={[styles.badgeLabel, { color: theme.onPrimary }]}>{count}</Text>
                    </View>
                  )}
                </View>
              )}
            </Pressable>
          );
        })}
      </View>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  scroll: { flexGrow: 1 },
  track: { borderRadius: Radius.pill, borderWidth: StyleSheet.hairlineWidth, flexDirection: 'row', flexGrow: 1, paddingHorizontal: 4 },
  tab: { flexGrow: 1, justifyContent: 'center', minHeight: 48 },
  segment: { alignItems: 'center', borderRadius: Radius.pill, flexDirection: 'row', gap: 6, justifyContent: 'center', minHeight: 38, paddingHorizontal: 14 },
  label: { fontSize: 14 },
  badge: { alignItems: 'center', borderRadius: Radius.pill, justifyContent: 'center', minHeight: 18, minWidth: 18, paddingHorizontal: 5 },
  badgeLabel: { fontSize: 11, fontVariant: ['tabular-nums'], fontWeight: '700', lineHeight: 14 },
});
