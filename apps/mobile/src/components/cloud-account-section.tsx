import { useState } from 'react';
import { Alert, Text, View } from 'react-native';
import { WorkbenchButton } from './workbench';
import { cloudAccount } from '@/lib/cloud-account';
import { useTheme } from '@/hooks/use-theme';
/** Account identity stays separate from disabled cloud compute services. */
export function CloudAccountSection() {
  const theme = useTheme();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function signOut() {
    setBusy(true); setError(null);
    try {
      const result = await cloudAccount.signOut();
      if (!result.revoked) Alert.alert('Signed out on this phone',
        'Remote revocation could not be confirmed. Review this device when you reconnect.');
    }
    catch { setError('Could not sign out. Try again.'); }
    finally { setBusy(false); }
  }
  return <View style={{ gap: 12 }}>
    <Text style={{ color: theme.text, fontSize: 16, fontWeight: '600' }}>Ion account</Text>
    <WorkbenchButton label={busy ? 'Signing out…' : 'Sign out'} disabled={busy} onPress={() => Alert.alert('Sign out?', 'Your saved desktops stay on this device.',
      [{ text: 'Cancel', style: 'cancel' }, { text: 'Sign out', onPress: () => void signOut() }])} />
    {error && <Text accessibilityRole="alert" style={{ color: theme.danger }}>{error}</Text>}
  </View>;
}
