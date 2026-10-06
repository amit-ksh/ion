import { useQuery } from '@tanstack/react-query';
import type { WakuClient } from '@waku/client';
import { useFocusEffect } from 'expo-router';
import { useCallback, useEffect, useRef, useState } from 'react';
import { AppState } from 'react-native';
import { useDaemon } from '@/lib/daemon-context';

// Unique across mounts: an old cached response must never match a new screen.
let nextVisibilityGeneration = 0;

/** Only visible, foreground screens poll. A cached owner projection never
 * enables controls until a fresh query on this connection has completed. */
export function useAutomationQuery<T>(key: readonly unknown[], load: (client: WakuClient) => Promise<T>) {
  const daemon = useDaemon();
  const [focused, setFocused] = useState(false);
  const [foreground, setForeground] = useState(AppState.currentState === 'active');
  const [requiredGeneration, setRequiredGeneration] = useState(() => ++nextVisibilityGeneration);
  const generation = useRef(requiredGeneration);
  useFocusEffect(useCallback(() => {
    generation.current = ++nextVisibilityGeneration;
    setRequiredGeneration(generation.current); setFocused(true);
    return () => setFocused(false);
  }, []));
  useEffect(() => {
    const subscription = AppState.addEventListener('change', (state) => {
      setForeground(state === 'active');
      if (state === 'active') {
        generation.current = ++nextVisibilityGeneration;
        setRequiredGeneration(generation.current);
      }
    });
    return () => subscription.remove();
  }, []);
  const enabled = focused && foreground && daemon.phase === 'connected' && Boolean(daemon.client);
  const query = useQuery({
    queryKey: ['daemon', daemon.activeProfile?.id ?? 'disconnected', 'automations', daemon.activeProfile?.address, daemon.connections, ...key],
    queryFn: async () => {
      const client = daemon.client;
      if (!client) throw new Error('Connect to the owner daemon to refresh.');
      const startedGeneration = generation.current;
      const value = await load(client);
      return { value, client, generation: startedGeneration };
    },
    enabled, retry: false, staleTime: 0,
    refetchInterval: enabled ? 8_000 : false,
    refetchIntervalInBackground: false,
  });
  useEffect(() => { if (enabled) void query.refetch(); }, [enabled, daemon.client, requiredGeneration]);
  return {
    query, data: query.data?.value, daemon, visible: focused && foreground,
    fresh: enabled && !query.isError && query.data?.client === daemon.client && query.data.generation === requiredGeneration,
  };
}
