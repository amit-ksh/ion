import type { ProviderKind } from '@waku/client';
import { StyleSheet, View } from 'react-native';

import { AppSymbol } from './app-symbol';
import { ProviderIcon } from './provider-icon';
import { useTheme } from '@/hooks/use-theme';

/** ready: harness installed; missing: harness (or the profile) is unavailable. */
export type AgentPresence = 'ready' | 'missing' | 'working' | 'needsYou' | 'idle';

/**
 * Round agent avatar carrying the harness mark, with a presence dot at the
 * bottom-right ringed in the surface color it sits on (Paperclip org chart).
 */
export function AgentAvatar({ provider, size = 36, presence, ring }: {
  provider?: ProviderKind | null;
  size?: number;
  presence?: AgentPresence | null;
  /** The card color behind the avatar, so the dot reads as cut out of it. */
  ring?: string;
}) {
  const theme = useTheme();
  const dot = size >= 36 ? 12 : 10;
  const color = presence === 'ready' || presence === 'working' ? theme.success
    : presence === 'needsYou' ? theme.primary
      : presence === 'missing' ? theme.warning : theme.textTertiary;
  return (
    <View style={{ width: size, height: size }}>
      <View style={[styles.avatar, { width: size, height: size, borderRadius: size / 2, backgroundColor: theme.surfaceMuted, borderColor: theme.border }]}>
        {provider ? <ProviderIcon provider={provider} size={Math.round(size * 0.5)} />
          : <AppSymbol name={{ android: 'error', web: 'error' }} size={Math.round(size * 0.5)} tintColor={theme.warning} />}
      </View>
      {presence && (
        <View style={[styles.dot, {
          width: dot, height: dot, borderRadius: dot / 2, right: -1, bottom: -1,
          backgroundColor: color, borderColor: ring ?? theme.ticket,
        }]} />
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  avatar: { alignItems: 'center', borderWidth: StyleSheet.hairlineWidth, justifyContent: 'center' },
  dot: { borderWidth: 2, position: 'absolute' },
});
