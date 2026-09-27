import { Stack } from 'expo-router';
import { useEffect, useState } from 'react';
import {
  ActivityIndicator, Alert, Pressable, ScrollView, StyleSheet, Text, TextInput, View,
} from 'react-native';

import { useTheme } from '@/hooks/use-theme';
import {
  decideGatewayProposal, gatewayReviewQueue, gatewayReviewStatus, pairGateway,
  type GatewayPairing, type GatewayProposal,
} from '@/lib/gateway-client';
import { deleteGatewayPairing, readGatewayPairing, writeGatewayPairing } from '@/lib/gateway-storage';

export default function GatewayScreen() {
  const theme = useTheme();
  const [pairing, setPairing] = useState<GatewayPairing | null>(null);
  const [url, setUrl] = useState('');
  const [invite, setInvite] = useState('');
  const [proposals, setProposals] = useState<GatewayProposal[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [online, setOnline] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void readGatewayPairing().then((saved) => {
      if (!active) return;
      setPairing(saved);
      setUrl(saved?.baseUrl ?? '');
      setLoaded(true);
      if (saved?.grants.includes('review')) void refresh(saved);
    }).catch((error) => {
      if (active) { setMessage(describe(error)); setLoaded(true); }
    });
    return () => { active = false; };
  }, []);

  async function refresh(current = pairing) {
    if (!current?.grants.includes('review')) return;
    setBusy(true);
    try {
      const queue = await gatewayReviewQueue(current);
      setProposals(queue);
      setOnline(true);
      setMessage(queue.length ? null : 'No actions need your review.');
    } catch (error) {
      setOnline(false);
      setMessage(`Gateway unavailable: ${describe(error)}. Decisions are paused.`);
    } finally { setBusy(false); }
  }

  async function pair() {
    if (busy) return;
    setBusy(true);
    setMessage(null);
    try {
      const result = await pairGateway(url, invite);
      await writeGatewayPairing(result);
      setPairing(result);
      setInvite('');
      setMessage('Phone paired. The device grant expires automatically; the owner can revoke it sooner.');
      if (result.grants.includes('review')) await refresh(result);
    } catch (error) { setMessage(describe(error)); }
    finally { setBusy(false); }
  }

  async function remove() {
    await deleteGatewayPairing();
    setPairing(null);
    setProposals([]);
    setOnline(false);
    setMessage('Pairing removed from this phone. Revoke the device in /owner too.');
  }

  function confirmRemove() {
    Alert.alert('Remove pairing?', 'The gateway owner should revoke this phone in /owner as well.', [
      { text: 'Cancel', style: 'cancel' },
      { text: 'Remove', style: 'destructive', onPress: () => void remove() },
    ]);
  }

  async function decide(proposal: GatewayProposal, approve: boolean) {
    if (!pairing || !online || busy) return;
    setBusy(true);
    try {
      const decided = await decideGatewayProposal(pairing, proposal, approve);
      setProposals((items) => items.filter((item) => item.id !== decided.id));
      setMessage(approve
        ? 'Exact action approved. The gateway owner must execute it separately.'
        : 'Action denied. No external write was made.');
    } catch (error) {
      // A dropped response is ambiguous. Read the durable decision before
      // offering another action; the gateway also makes identical retries safe.
      try {
        const current = await gatewayReviewStatus(pairing, proposal.id);
        if (current.status !== 'pending') {
          setProposals((items) => items.filter((item) => item.id !== current.id));
          setMessage(`Gateway recorded: ${current.status}.`);
        } else setMessage(describe(error));
      } catch {
        setOnline(false);
        setMessage('Decision status is unknown. Reconnect before reviewing another action.');
      }
    } finally { setBusy(false); }
  }

  function confirmDecision(proposal: GatewayProposal, approve: boolean) {
    const effect = proposal.effect;
    Alert.alert(
      approve ? 'Approve exact action?' : 'Deny action?',
      `${effect.tool} on ${effect.accountId}\nTarget: ${effect.target}\nVersion: ${effect.version}\nReview the full arguments and scopes on this screen before deciding.`,
      [
        { text: 'Cancel', style: 'cancel' },
        { text: approve ? 'Approve' : 'Deny', style: approve ? 'default' : 'destructive', onPress: () => void decide(proposal, approve) },
      ],
    );
  }

  return (
    <ScrollView style={{ backgroundColor: theme.background }} contentContainerStyle={styles.page} keyboardShouldPersistTaps="handled">
      <Stack.Screen options={{ title: 'Personal gateway' }} />
      <Text style={[styles.title, { color: theme.text }]}>Personal gateway</Text>
      <Text style={[styles.help, { color: theme.textSecondary }]}>
        Pair this phone with a one-use invite from the gateway owner. A review grant lets you decide exact pending actions. Only the owner can execute an approved action.
      </Text>
      {!loaded ? <ActivityIndicator accessibilityLabel="Loading gateway pairing" /> : pairing ? (
        <View style={styles.section}>
          <Text style={[styles.meta, { color: theme.text }]}>Connected to {pairing.baseUrl}</Text>
          <Text style={[styles.meta, { color: theme.textSecondary }]}>Grants: {pairing.grants.join(', ')} · Expires {new Date(pairing.expiresAt).toLocaleString()}</Text>
          <View style={styles.actions}>
            <Action label={busy ? 'Checking…' : 'Refresh approvals'} disabled={busy || !pairing.grants.includes('review')} onPress={() => void refresh()} color={theme.accent} />
            <Action label="Remove pairing" disabled={busy} onPress={confirmRemove} color={theme.danger} />
          </View>
          {!pairing.grants.includes('review') && <Text style={[styles.help, { color: theme.textSecondary }]}>This device has no review grant. Create a new invite with Review approvals in /owner to enable this screen.</Text>}
        </View>
      ) : (
        <View style={styles.section}>
          <Text style={[styles.label, { color: theme.text }]}>Gateway HTTPS URL</Text>
          <TextInput accessibilityLabel="Gateway HTTPS URL" autoCapitalize="none" autoCorrect={false} inputMode="url" value={url} onChangeText={setUrl} placeholder="https://your-gateway.example" placeholderTextColor={theme.textTertiary} style={[styles.input, { color: theme.text, borderColor: theme.borderStrong, backgroundColor: theme.surface }]} />
          <Text style={[styles.label, { color: theme.text }]}>One-use invite</Text>
          <TextInput accessibilityLabel="One-use pairing invite" autoCapitalize="none" autoCorrect={false} secureTextEntry value={invite} onChangeText={setInvite} placeholder="Paste invite from /owner" placeholderTextColor={theme.textTertiary} style={[styles.input, { color: theme.text, borderColor: theme.borderStrong, backgroundColor: theme.surface }]} />
          <Action label={busy ? 'Pairing…' : 'Pair phone'} disabled={busy || !url.trim() || !invite.trim()} onPress={() => void pair()} color={theme.accent} />
        </View>
      )}
      {message && <Text accessibilityRole="alert" style={[styles.message, { color: theme.textSecondary }]}>{message}</Text>}
      {pairing?.grants.includes('review') && proposals.map((proposal) => {
        const effect = proposal.effect;
        return (
          <View key={proposal.id} style={[styles.card, { backgroundColor: theme.raised, borderColor: theme.borderStrong }]}>
            <Text style={[styles.tool, { color: theme.text }]}>{effect.tool}</Text>
            <Text style={[styles.meta, { color: theme.textSecondary }]}>Proposed by {proposal.deviceId}</Text>
            <Text style={[styles.meta, { color: theme.text }]}>Toolkit: {effect.toolkit} · Version: {effect.version}</Text>
            <Text style={[styles.meta, { color: theme.text }]}>Account: {effect.accountId}</Text>
            <Text style={[styles.meta, { color: theme.text }]}>Target: {effect.target}</Text>
            <Text style={[styles.meta, { color: theme.text }]}>Scopes: {effect.scopes.join(', ') || 'None'}</Text>
            <Text style={[styles.label, { color: theme.text }]}>Exact JSON arguments</Text>
            <Text selectable style={[styles.body, { color: theme.text, backgroundColor: theme.inset }]}>{effect.body}</Text>
            <Text selectable style={[styles.fingerprint, { color: theme.textSecondary }]}>Fingerprint: {proposal.fingerprint}</Text>
            <View style={styles.actions}>
              <Action label="Deny" disabled={busy || !online} onPress={() => confirmDecision(proposal, false)} color={theme.danger} />
              <Action label="Approve exact action" disabled={busy || !online} onPress={() => confirmDecision(proposal, true)} color={theme.accent} />
            </View>
          </View>
        );
      })}
    </ScrollView>
  );
}

function Action({ label, disabled, onPress, color }: { label: string; disabled: boolean; onPress: () => void; color: string }) {
  return <Pressable accessibilityRole="button" accessibilityState={{ disabled }} disabled={disabled} onPress={onPress} style={[styles.button, { borderColor: color, opacity: disabled ? 0.5 : 1 }]}><Text style={{ color, fontWeight: '600' }}>{label}</Text></Pressable>;
}

function describe(error: unknown): string { return error instanceof Error ? error.message : 'Unknown gateway error'; }

const styles = StyleSheet.create({
  page: { padding: 20, paddingBottom: 70, gap: 18 },
  title: { fontSize: 24, fontWeight: '700' },
  help: { fontSize: 14, lineHeight: 21 },
  section: { gap: 10 },
  label: { fontSize: 13, fontWeight: '600', marginTop: 4 },
  input: { borderWidth: 1, borderRadius: 9, minHeight: 48, paddingHorizontal: 12, fontSize: 15 },
  button: { alignItems: 'center', justifyContent: 'center', minHeight: 44, borderWidth: 1, borderRadius: 9, paddingHorizontal: 14, paddingVertical: 10 },
  actions: { flexDirection: 'row', flexWrap: 'wrap', gap: 9, marginTop: 6 },
  message: { fontSize: 13, lineHeight: 19 },
  card: { borderWidth: 1, borderRadius: 12, padding: 15, gap: 7 },
  tool: { fontSize: 17, fontWeight: '700' },
  meta: { fontSize: 13, lineHeight: 19 },
  body: { fontFamily: 'monospace', fontSize: 12, padding: 10, borderRadius: 6 },
  fingerprint: { fontSize: 11, lineHeight: 16 },
});
