import type { ProviderKind } from '@waku/client';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { AppSymbol } from './app-symbol';
import { ProviderIcon } from './provider-icon';
import { StatusPill, type TicketStatus } from './ticket-card';
import { WorkbenchButton } from './workbench';
import { Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

type SymbolName = Parameters<typeof AppSymbol>[0]['name'];
export type ApprovalAction = { label: string; onPress: () => void; disabled?: boolean };

/**
 * A request waiting on the user (Paperclip ApprovalCard): round type icon,
 * kind badge, requester, title and age, a status pill on the right, the
 * proposal body, then a footer with the resolving actions.
 */
export function ApprovalCard({ kind, icon, requester, title, time, status, detail, approve, reject, open }: {
  kind: string;
  icon: SymbolName;
  requester?: { name: string; provider?: ProviderKind | null } | null;
  title: string;
  /** Relative age, e.g. "15m ago". */
  time?: string | null;
  status: TicketStatus;
  detail?: string | null;
  approve?: ApprovalAction | null;
  reject?: ApprovalAction | null;
  open?: (() => void) | null;
}) {
  const theme = useTheme();
  const summary = [kind, title, requester ? `Requested by ${requester.name}` : null, time, status.label].filter(Boolean).join(', ');
  return (
    <View style={[styles.card, { backgroundColor: theme.ticket, borderColor: theme.ticketBorder, boxShadow: theme.elevationRest }]}>
      <Pressable accessibilityRole={open ? 'button' : undefined} accessibilityLabel={summary}
        accessibilityHint={open ? 'Opens the full request' : undefined} disabled={!open} onPress={open ?? undefined}
        style={({ pressed }) => [styles.head, pressed && { backgroundColor: theme.overlay }]}>
        <View style={[styles.typeIcon, { backgroundColor: theme.background, borderColor: theme.borderStrong }]}>
          <AppSymbol name={icon} size={17} tintColor={theme.textSecondary} />
        </View>
        <View style={styles.copy}>
          <View style={styles.badges}>
            <View style={[styles.kind, { borderColor: theme.borderStrong, backgroundColor: theme.background }]}>
              <Text style={[styles.kindLabel, { color: theme.textSecondary }]}>{kind.toUpperCase()}</Text>
            </View>
            {requester && (
              <View style={styles.requester}>
                <Text style={[styles.small, { color: theme.textSecondary }]}>Requested by</Text>
                {requester.provider && <ProviderIcon provider={requester.provider} size={13} />}
                <Text numberOfLines={1} style={[styles.small, styles.requesterName, { color: theme.text }]}>{requester.name}</Text>
              </View>
            )}
          </View>
          <Text style={[styles.title, { color: theme.text }]}>{title}</Text>
          {time ? <Text style={[styles.small, { color: theme.textTertiary }]}>Requested {time}</Text> : null}
        </View>
        <StatusPill {...status} />
      </Pressable>
      {detail ? (
        <View style={[styles.section, { borderTopColor: theme.border }]}>
          <Text numberOfLines={4} style={[styles.detail, { color: theme.textSecondary }]}>{detail}</Text>
        </View>
      ) : null}
      {(approve || reject || open) && (
        <View style={[styles.section, styles.footer, { borderTopColor: theme.border }]}>
          {approve && <WorkbenchButton label={approve.label} primary icon={{ ios: 'checkmark', android: 'check', web: 'check' }} disabled={approve.disabled} onPress={approve.onPress} />}
          {reject && <WorkbenchButton label={reject.label} destructive disabled={reject.disabled} onPress={reject.onPress} />}
          <View style={styles.grow} />
          {open && (
            <Pressable accessibilityRole="button" accessibilityLabel={`View details for ${title}`} onPress={open}
              style={({ pressed }) => [styles.details, { backgroundColor: pressed ? theme.overlayStrong : 'transparent' }]}>
              <Text style={[styles.detailsLabel, { color: theme.textSecondary }]}>View details</Text>
            </Pressable>
          )}
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  card: { borderRadius: Radius.medium, borderWidth: 1, overflow: 'hidden' },
  head: { alignItems: 'flex-start', flexDirection: 'row', gap: 12, padding: 16 },
  typeIcon: { alignItems: 'center', borderRadius: 18, borderWidth: StyleSheet.hairlineWidth, height: 36, justifyContent: 'center', width: 36 },
  copy: { flex: 1, gap: 6, minWidth: 0 },
  badges: { alignItems: 'flex-start', gap: 8 },
  kind: { borderRadius: Radius.pill, borderWidth: StyleSheet.hairlineWidth, paddingHorizontal: 8, paddingVertical: 2 },
  kindLabel: { fontSize: 10.5, fontWeight: '600', letterSpacing: 0.6, lineHeight: 14 },
  requester: { alignItems: 'center', alignSelf: 'stretch', flexDirection: 'row', gap: 5, minWidth: 0 },
  requesterName: { flexShrink: 1, fontWeight: '500' },
  small: { fontSize: 12, lineHeight: 17 },
  title: { fontSize: 16, fontWeight: '600', lineHeight: 22 },
  section: { borderTopWidth: StyleSheet.hairlineWidth, marginHorizontal: 16, paddingVertical: 14 },
  detail: { fontSize: 14, lineHeight: 21 },
  footer: { alignItems: 'center', flexDirection: 'row', flexWrap: 'wrap', gap: 8, paddingBottom: 16 },
  grow: { flexGrow: 1 },
  details: { borderRadius: Radius.small, justifyContent: 'center', minHeight: 48, paddingHorizontal: 10 },
  detailsLabel: { fontSize: 13, fontWeight: '500' },
});
