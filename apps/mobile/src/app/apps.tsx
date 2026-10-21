import * as Crypto from 'expo-crypto';
import { useRef, useState } from 'react';
import { ActivityIndicator, Alert, FlatList, Linking, StyleSheet, Text, View } from 'react-native';
import { AutomationField } from '@/components/automation-form-controls';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { WorkbenchButton, WorkbenchCard, WorkbenchState } from '@/components/workbench';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTheme } from '@/hooks/use-theme';
import { appConnectionPending, appConnectionUrl, parseAppsCatalog, type CatalogApp } from '@/lib/apps-catalog';
import { cloudAccount } from '@/lib/cloud-account';

export default function AppsScreen() {
  const theme = useTheme();
  const header = useScreenHeaderInset();
  const [draft, setDraft] = useState('');
  const [search, setSearch] = useState('');
  const [connected, setConnected] = useState(false);
  const [after, setAfter] = useState<string | null>(null);
  const [previous, setPrevious] = useState<(string | null)[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const locked = useRef(false);
  const operations = useRef(new Map<string, string>());
  const links = useRef(new Map<string, { account: string; url: string }>());
  const state = useAutomationQuery(['managed-apps', search, connected, after], async () => {
    const before = await cloudAccount.verifySession();
    if (!before.signedIn || !before.workspaceId) throw new Error('Sign in to manage app connections.');
    const catalog = parseAppsCatalog(await cloudAccount.request('/v1/apps/catalog', 'POST', { search, connectedOnly: connected, cursor: after ?? '' }));
    const current = await cloudAccount.status();
    if (!current.signedIn || current.workspaceId !== before.workspaceId) throw new Error('Your workspace changed. Refresh to continue.');
    return { catalog, workspace: before.workspaceId };
  }, { poll: false });
  async function operate(app: CatalogApp, disconnect: boolean) {
    if (!state.fresh || !state.data || locked.current) return;
    locked.current = true; setBusy(app.toolkit); setFeedback(null);
    try {
      const current = await cloudAccount.verifySession();
      if (!current.signedIn || current.workspaceId !== state.data.workspace) throw new Error('Your workspace changed. Refresh to continue.');
      const key = disconnect ? `disconnect:${app.account!.id}` : `connect:${app.toolkit}`;
      const operationId = operations.current.get(key) ?? Crypto.randomUUID();
      operations.current.set(key, operationId);
      const result = await cloudAccount.request(disconnect ? '/v1/apps/disconnect' : '/v1/apps/connect', 'POST', {
        ...(disconnect ? { accountId: app.account!.id } : { toolkit: app.toolkit }), operationId,
      });
      const now = await cloudAccount.status();
      if (now.workspaceId !== current.workspaceId || !now.signedIn) return;
      if (result?.account?.toolkit !== app.toolkit || !result.account.id) throw new Error('Connection was not confirmed. Refresh before trying again.');
      if (disconnect) {
        if (result.account.id !== app.account?.id || result.account.status !== 'REVOKED') throw new Error('Disconnect was not confirmed. Refresh before trying again.');
        operations.current.delete(key);
        links.current.delete(app.toolkit);
        setFeedback(`${app.name} disconnected.`);
      } else {
        const url = appConnectionUrl(result.redirectUrl);
        links.current.set(app.toolkit, { account: result.account.id, url });
        await Linking.openURL(url);
        setFeedback(`Complete ${app.name} sign-in in your browser, then refresh its connection status.`);
      }
      await state.query.refetch();
    } catch (cause) {
      setFeedback(cause instanceof Error ? cause.message : 'Could not update connection. Refresh its status before retrying.');
    } finally { locked.current = false; setBusy(null); }
  }
  return <FlatList style={{ backgroundColor: theme.background }} data={state.data?.catalog.items ?? []} keyExtractor={item => item.toolkit}
    contentContainerStyle={[styles.content, { paddingTop: header + 16 }]}
    refreshing={state.query.isFetching && Boolean(state.data)} onRefresh={() => void state.query.refetch()}
    ListHeaderComponent={<View style={styles.header}>
      <ConnectionBanner />
      <AutomationField label="Find an app" value={draft} onChangeText={setDraft} maxLength={256} autoCapitalize="none" returnKeyType="search"
        onSubmitEditing={() => { setSearch(draft.trim()); setAfter(null); setPrevious([]); }} />
      <View style={styles.actions}><WorkbenchButton label="Search" onPress={() => { setSearch(draft.trim()); setAfter(null); setPrevious([]); }} />
        <WorkbenchButton label={connected ? 'Show all apps' : 'Connected apps'} onPress={() => { setConnected(!connected); setAfter(null); setPrevious([]); }} />
        <WorkbenchButton label="Refresh" disabled={state.query.isFetching || Boolean(busy)} onPress={() => void state.query.refetch()} /></View>
      {feedback && <Text accessibilityLiveRegion="polite" style={[styles.detail, { color: theme.textSecondary }]}>{feedback}</Text>}
      {state.query.isError && <WorkbenchState title="Apps unavailable" detail={state.query.error.message} />}
    </View>}
    ListEmptyComponent={state.daemon.phase !== 'connected' ? <WorkbenchState title="Connect to your desktop" detail="Reconnect from Settings, then refresh app connections." /> : state.query.isFetching ? <ActivityIndicator accessibilityLabel="Loading apps" color={theme.accent} />
      : !state.query.isError ? <WorkbenchState title={connected ? 'No connected apps' : 'No matching apps'} detail="Choose an app to connect your account." /> : null}
    renderItem={({ item }) => {
      const active = item.account?.status === 'ACTIVE';
      const pending = appConnectionPending(item);
      const continuation = links.current.get(item.toolkit);
      const canContinue = pending && continuation?.account === item.account?.id;
      return <View style={styles.item}><WorkbenchCard>
        <View style={styles.appHeading}><Text style={[styles.name, { color: theme.text }]}>{item.name}</Text>
          <Text style={[styles.status, { color: active ? theme.accent : theme.textSecondary }]}>{active ? 'Connected' : pending ? 'Sign-in pending' : item.account ? item.account.status : 'Not connected'}</Text></View>
        {item.description && <Text numberOfLines={3} style={[styles.detail, { color: theme.textSecondary }]}>{item.description}</Text>}
        {pending && <Text style={[styles.detail, { color: theme.textSecondary }]}>Finish browser sign-in, then refresh the connection status.</Text>}
        <View style={styles.actions}><WorkbenchButton label={busy === item.toolkit ? 'Updating…' : active ? 'Disconnect' : pending ? canContinue ? 'Continue sign-in' : 'Pending' : item.connectable ? 'Connect' : 'Unavailable'} primary={!active && !pending && item.connectable}
          disabled={Boolean(busy) || !state.fresh || (!active && !item.connectable) || pending && !canContinue} onPress={() => {
            if (canContinue && continuation) { void Linking.openURL(continuation.url).catch(() => setFeedback('Could not open browser sign-in. Refresh and retry.')); return; }
            if (active) Alert.alert(`Disconnect ${item.name}?`, 'Agents will lose access to this app account.', [{ text: 'Cancel', style: 'cancel' }, { text: 'Disconnect', style: 'destructive', onPress: () => void operate(item, true) }]);
            else void operate(item, false);
          }} /></View>
      </WorkbenchCard></View>;
    }}
    ListFooterComponent={<View style={styles.actions}>
      {previous.length > 0 && <WorkbenchButton label="Previous" onPress={() => { setAfter(previous.at(-1) ?? null); setPrevious(previous.slice(0, -1)); }} />}
      {state.data?.catalog.nextCursor && <WorkbenchButton label="Next" onPress={() => { setPrevious([...previous, after]); setAfter(state.data!.catalog.nextCursor); }} />}
    </View>} />;
}
const styles = StyleSheet.create({ content: { width: '100%', maxWidth: 800, alignSelf: 'center', padding: 16, paddingBottom: 32 }, header: { gap: 12, marginBottom: 20 }, actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8 }, item: { marginBottom: 12 }, appHeading: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, alignItems: 'center', justifyContent: 'space-between' }, name: { fontSize: 17, lineHeight: 24, fontWeight: '600' }, status: { fontSize: 12, lineHeight: 18, fontWeight: '600' }, detail: { fontSize: 14, lineHeight: 21 } });
